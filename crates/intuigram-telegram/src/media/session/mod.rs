use std::collections::HashMap;
use std::net::SocketAddr;
use std::num::NonZeroUsize;

use compio::runtime::{JoinHandle, ResumeUnwind};
use compio_mtproto::{InvocationHandle, Route};
use futures_util::StreamExt as _;

use super::*;

mod cdn;

use cdn::CdnSessions;

enum Command {
    Connection {
        dc_id: i32,
        reply: flume::Sender<Result<InvocationHandle>>,
    },
    CdnFile {
        dc_id: i32,
        file_token: Vec<u8>,
        offset: i64,
        limit: i32,
        reply: flume::Sender<Result<tl::enums::upload::CdnFile>>,
    },
    Shutdown {
        reply: flume::Sender<()>,
    },
}

pub(crate) struct MediaSessionConfig {
    pub(crate) primary_dc: i32,
    pub(crate) primary: InvocationHandle,
    pub(crate) primary_session: Session,
    pub(crate) data_centers: DataCenterEndpoints,
    pub(crate) media_data_centers: DataCenterEndpoints,
    pub(crate) cdn_data_centers: DataCenterEndpoints,
    pub(crate) credentials: ApplicationCredentials,
    pub(crate) route: Route,
    pub(crate) capacity: NonZeroUsize,
}

#[derive(Clone)]
pub(crate) struct MediaSessions {
    sender: flume::Sender<Command>,
}

impl MediaSessions {
    pub(crate) fn start(config: MediaSessionConfig) -> Self {
        let (sender, receiver) = flume::bounded(config.capacity.get());
        compio::runtime::spawn(run(receiver, config)).detach();
        Self { sender }
    }

    pub(crate) async fn connection(&self, dc_id: i32) -> Result<InvocationHandle> {
        let (reply, response) = flume::bounded(1);
        self.sender
            .send_async(Command::Connection { dc_id, reply })
            .await
            .map_err(|_| Error::MediaSessionUnavailable)?;
        response
            .recv_async()
            .await
            .map_err(|_| Error::MediaSessionUnavailable)?
    }

    pub(crate) async fn shutdown(&self) {
        let (reply, response) = flume::bounded(1);
        if self
            .sender
            .send_async(Command::Shutdown { reply })
            .await
            .is_ok()
        {
            response.recv_async().await.ok();
        }
    }

    pub(crate) async fn cdn_file(
        &self,
        dc_id: i32,
        file_token: Vec<u8>,
        offset: i64,
        limit: i32,
    ) -> Result<tl::enums::upload::CdnFile> {
        let (reply, response) = flume::bounded(1);
        self.sender
            .send_async(Command::CdnFile {
                dc_id,
                file_token,
                offset,
                limit,
                reply,
            })
            .await
            .map_err(|_| Error::MediaSessionUnavailable)?;
        response
            .recv_async()
            .await
            .map_err(|_| Error::MediaSessionUnavailable)?
    }
}

async fn run(receiver: flume::Receiver<Command>, config: MediaSessionConfig) {
    let mut connections = HashMap::<i32, InvocationHandle>::new();
    let mut cdn = CdnSessions::new();
    let mut drivers = Vec::<JoinHandle<()>>::new();
    let mut shutdown = None;
    while let Ok(command) = receiver.recv_async().await {
        match command {
            Command::Connection { dc_id, reply } => {
                let result = match connections.get(&dc_id) {
                    Some(connection) => Ok(connection.clone()),
                    None => connect(dc_id, &config).await.map(|(connection, driver)| {
                        connections.insert(dc_id, connection.clone());
                        drivers.push(driver);
                        connection
                    }),
                };
                reply.send(result).ok();
            }
            Command::CdnFile {
                dc_id,
                file_token,
                offset,
                limit,
                reply,
            } => {
                let result = cdn.file(dc_id, file_token, offset, limit, &config).await;
                reply.send(result).ok();
            }
            Command::Shutdown { reply } => {
                shutdown = Some(reply);
                break;
            }
        }
    }
    for connection in connections.values() {
        connection.stop();
    }
    connections.clear();
    cdn.shutdown().await;
    for driver in drivers {
        driver.await.resume_unwind();
    }
    if let Some(reply) = shutdown {
        reply.send(()).ok();
    }
}

async fn connect(
    dc_id: i32,
    config: &MediaSessionConfig,
) -> Result<(InvocationHandle, JoinHandle<()>)> {
    let endpoints = media_endpoints(dc_id, &config.data_centers, &config.media_data_centers)
        .context(MediaDataCenterUnavailableSnafu { dc_id })?;
    let client = if dc_id == config.primary_dc {
        Client::connect_with_session_endpoints(
            config.credentials.clone(),
            &config.primary_session,
            endpoints,
            None,
            config.route.clone(),
        )
        .await?
    } else {
        let exported = config
            .primary
            .invoke(&tl::functions::auth::ExportAuthorization { dc_id })
            .await
            .context(InvokeSnafu)?;
        let tl::enums::auth::ExportedAuthorization::Authorization(exported) = exported;
        let (mut client, _) = Client::connect_new_media(
            dc_id,
            endpoints,
            config.credentials.clone(),
            config.route.clone(),
        )
        .await?;
        client
            .connection
            .invoke(&tl::functions::auth::ImportAuthorization {
                id: exported.id,
                bytes: exported.bytes,
            })
            .await
            .context(InvokeSnafu)?;
        client
    };
    let (client, mut updates, _) = client.into_media_live(config.capacity);
    let Connection::Live(connection) = client.connection else {
        return MediaSessionUnavailableSnafu.fail();
    };
    let retained = connection.clone();
    let driver = compio::runtime::spawn(async move { while updates.next().await.is_some() {} });
    Ok((retained, driver))
}

fn media_endpoints<'a>(
    dc_id: i32,
    data_centers: &'a DataCenterEndpoints,
    media_data_centers: &'a DataCenterEndpoints,
) -> Option<&'a [SocketAddr]> {
    media_data_centers
        .get(&dc_id)
        .or_else(|| data_centers.get(&dc_id))
        .map(Vec::as_slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_only_endpoints_are_preferred() {
        let ordinary = "149.154.167.51:443"
            .parse()
            .expect("ordinary endpoint should parse");
        let media = "149.154.167.52:443"
            .parse()
            .expect("media endpoint should parse");

        assert_eq!(
            media_endpoints(
                4,
                &HashMap::from([(4, vec![ordinary])]),
                &HashMap::from([(4, vec![media])])
            ),
            Some([media].as_slice())
        );
    }

    #[test]
    fn ordinary_endpoints_are_safe_fallback() {
        let ordinary = "149.154.167.51:443"
            .parse()
            .expect("ordinary endpoint should parse");

        assert_eq!(
            media_endpoints(4, &HashMap::from([(4, vec![ordinary])]), &HashMap::new()),
            Some([ordinary].as_slice())
        );
    }
}
