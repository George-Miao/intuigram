use super::*;

struct CdnConnection {
    invocation: InvocationHandle,
    initialized: bool,
}

pub(super) struct CdnSessions {
    connections: HashMap<i32, CdnConnection>,
    keys: Option<HashMap<i32, Vec<String>>>,
    drivers: Vec<JoinHandle<()>>,
}

impl CdnSessions {
    pub(super) fn new() -> Self {
        Self {
            connections: HashMap::new(),
            keys: None,
            drivers: Vec::new(),
        }
    }

    pub(super) async fn file(
        &mut self,
        dc_id: i32,
        file_token: Vec<u8>,
        offset: i64,
        limit: i32,
        config: &MediaSessionConfig,
    ) -> Result<tl::enums::upload::CdnFile> {
        for retry in 0..=1 {
            if let std::collections::hash_map::Entry::Vacant(entry) = self.connections.entry(dc_id)
            {
                let (connection, driver) = connect(dc_id, &mut self.keys, config).await?;
                entry.insert(connection);
                self.drivers.push(driver);
            }
            let connection = self
                .connections
                .get_mut(&dc_id)
                .expect("a missing CDN session is connected before invocation");
            let request = tl::functions::upload::GetCdnFile {
                file_token: file_token.clone(),
                offset,
                limit,
            };
            let result = if connection.initialized {
                connection.invocation.invoke(&request).await
            } else {
                initialize(connection, request, config).await
            };
            if retry == 0 && matches!(&result, Err(InvocationError::Rpc { code: -404, .. })) {
                if let Some(connection) = self.connections.remove(&dc_id) {
                    connection.invocation.stop();
                }
                continue;
            }
            return result.context(InvokeSnafu);
        }
        unreachable!("the bounded CDN authorization retry always returns")
    }

    pub(super) async fn shutdown(&mut self) {
        for connection in self.connections.values() {
            connection.invocation.stop();
        }
        self.connections.clear();
        for driver in self.drivers.drain(..) {
            driver.await.resume_unwind();
        }
    }
}

async fn initialize(
    connection: &mut CdnConnection,
    request: tl::functions::upload::GetCdnFile,
    config: &MediaSessionConfig,
) -> std::result::Result<tl::enums::upload::CdnFile, InvocationError> {
    let result = connection
        .invocation
        .invoke(&tl::functions::InvokeWithLayer {
            layer: tl::LAYER,
            query: tl::functions::InitConnection {
                api_id: config.credentials.api_id,
                device_model: "Intuigram CDN".to_owned(),
                system_version: "unknown".to_owned(),
                app_version: env!("CARGO_PKG_VERSION").to_owned(),
                system_lang_code: "en".to_owned(),
                lang_pack: String::new(),
                lang_code: "en".to_owned(),
                proxy: None,
                params: None,
                query: request,
            },
        })
        .await;
    if match &result {
        Ok(_) => true,
        Err(error) => !error.is_connection_failure(),
    } {
        connection.initialized = true;
    }
    result
}

async fn connect(
    dc_id: i32,
    keys: &mut Option<HashMap<i32, Vec<String>>>,
    config: &MediaSessionConfig,
) -> Result<(CdnConnection, JoinHandle<()>)> {
    if keys.is_none() {
        let tl::enums::CdnConfig::Config(cdn_config) = config
            .primary
            .invoke(&tl::functions::help::GetCdnConfig {})
            .await
            .context(InvokeSnafu)?;
        let mut grouped = HashMap::<i32, Vec<String>>::new();
        for key in cdn_config.public_keys {
            let tl::enums::CdnPublicKey::Key(key) = key;
            grouped.entry(key.dc_id).or_default().push(key.public_key);
        }
        *keys = Some(grouped);
    }
    let public_keys = keys
        .as_ref()
        .and_then(|keys| keys.get(&dc_id))
        .context(CdnPublicKeysUnavailableSnafu { dc_id })?;
    let endpoints = config
        .cdn_data_centers
        .get(&dc_id)
        .map(Vec::as_slice)
        .context(CdnDataCenterUnavailableSnafu { dc_id })?;
    let (mut transport, _) = connect_route(endpoints, -dc_id, &config.route)
        .await
        .with_context(|_| ConnectSnafu {
            endpoints: endpoints.to_vec(),
        })?;
    let material = generate_auth_key_with_rsa_keys(&mut transport, public_keys)
        .await
        .context(GenerateKeySnafu)?;
    let encrypted = EncryptedConnection::from_boxed(transport, &material);
    let (invocation, mut updates, driver) = encrypted.into_driver(config.capacity);
    let driver = compio::runtime::spawn(async move {
        let drain = async move { while updates.next().await.is_some() {} };
        futures_util::pin_mut!(driver, drain);
        futures_util::future::select(driver, drain).await;
    });
    Ok((
        CdnConnection {
            invocation,
            initialized: false,
        },
        driver,
    ))
}
