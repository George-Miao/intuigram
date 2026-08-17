use super::*;

mod transport;

impl Client {
    /// Stops retained media data-center sessions after admitted transfers
    /// drain during Account shutdown.
    pub async fn shutdown_media_sessions(&self) {
        if let Some(sessions) = &self.media_sessions {
            sessions.shutdown().await;
        }
    }

    /// Forks independently mutable media metadata around the live Account's
    /// cloneable, correlated invocation endpoint.
    pub fn media_client(&self) -> Result<MediaClient> {
        let Connection::Live(connection) = &self.connection else {
            return MediaSessionUnavailableSnafu.fail();
        };
        Ok(MediaClient(Self {
            connection: Connection::Live(connection.clone()),
            session: self.session.clone(),
            dc_id: self.dc_id,
            route: self.route.clone(),
            credentials: self.credentials.clone(),
            password: self.password.clone(),
            identity: self.identity.clone(),
            peers: self.peers.clone(),
            names: self.names.clone(),
            channel_pts: self.channel_pts.clone(),
            data_centers: self.data_centers.clone(),
            media_data_centers: self.media_data_centers.clone(),
            cdn_data_centers: self.cdn_data_centers.clone(),
            media_sessions: self.media_sessions.clone(),
            venue_search_username: self.venue_search_username.clone(),
            venue_search_bot: self.venue_search_bot.clone(),
        }))
    }

    /// Converts an authenticated sequential connection into a cloneable raw
    /// invocation endpoint and continuously driven update stream.
    #[must_use]
    pub fn into_live(self, capacity: NonZeroUsize) -> (Self, LiveUpdates, PeerDirectory) {
        let (mut client, updates, directory) = self.into_live_without_media_sessions(capacity);
        let Connection::Live(handle) = &client.connection else {
            unreachable!("the live client was constructed with a live endpoint")
        };
        client.media_sessions = Some(MediaSessions::start(MediaSessionConfig {
            primary_dc: client.dc_id,
            primary: handle.clone(),
            primary_session: client.session.clone(),
            data_centers: client.data_centers.clone(),
            media_data_centers: client.media_data_centers.clone(),
            cdn_data_centers: client.cdn_data_centers.clone(),
            credentials: client.credentials.clone(),
            route: client.route.clone(),
            capacity,
        }));
        (client, updates, directory)
    }

    pub(crate) fn into_media_live(
        self,
        capacity: NonZeroUsize,
    ) -> (Self, LiveUpdates, PeerDirectory) {
        self.into_live_without_media_sessions(capacity)
    }

    fn into_live_without_media_sessions(
        self,
        capacity: NonZeroUsize,
    ) -> (Self, LiveUpdates, PeerDirectory) {
        let Self {
            connection,
            session,
            dc_id,
            route,
            credentials,
            password,
            identity,
            peers,
            names,
            channel_pts,
            data_centers,
            media_data_centers,
            cdn_data_centers,
            media_sessions: _,
            venue_search_username,
            venue_search_bot,
        } = self;
        let directory = peers.clone();
        let Connection::Login(connection) = connection else {
            unreachable!("a Telegram client enters live mode only once after login")
        };
        let (handle, updates, driver) = (*connection).into_driver(capacity);
        let client = Self {
            connection: Connection::Live(handle),
            session,
            dc_id,
            route,
            credentials,
            password,
            identity,
            peers,
            names: names.clone(),
            channel_pts,
            data_centers,
            media_data_centers,
            cdn_data_centers,
            media_sessions: None,
            venue_search_username,
            venue_search_bot,
        };
        (
            client,
            LiveUpdates {
                driver: Box::pin(driver),
                updates,
                names,
                terminated: false,
            },
            directory,
        )
    }

    /// Adds operation addresses learned by the live update stream.
    pub fn merge_peers(&mut self, peers: PeerDirectory) {
        self.peers.merge(peers);
    }

    /// Returns the current avatar revision for one cached peer.
    #[must_use]
    pub fn avatar_ref(&self, peer: intuigram_lib::ChatId) -> Option<intuigram_lib::AvatarRef> {
        self.peers.avatar_ref(peer)
    }

    /// Returns the route policy for data-center migrations and media downloads.
    #[must_use]
    pub fn connection_route(&self) -> Route {
        self.route.clone()
    }
}

fn route_attempts(route: &Route) -> Vec<Route> {
    let mut attempts = route
        .proxies
        .iter()
        .cloned()
        .map(|proxy| Route {
            proxies: vec![proxy],
            direct_fallback: false,
            timeout: route.timeout,
        })
        .collect::<Vec<_>>();
    if route.direct_fallback {
        attempts.push(Route {
            proxies: Vec::new(),
            direct_fallback: true,
            timeout: route.timeout,
        });
    }
    if attempts.is_empty() {
        attempts.push(route.clone());
    }
    attempts
}

#[cfg(test)]
mod tests {
    use compio_mtproto::{DnsStrategy, Proxy, ProxyEndpoint, Route};

    use super::route_attempts;

    #[test]
    fn telegram_validation_receives_each_proxy_then_direct_fallback() {
        let proxy = Proxy::Socks5 {
            endpoint: ProxyEndpoint {
                host: "proxy.example".to_owned(),
                port: 1080,
            },
            credentials: None,
            dns: DnsStrategy::Remote,
        };
        let route = Route {
            proxies: vec![proxy.clone(), proxy],
            ..Route::default()
        };

        let attempts = route_attempts(&route);

        assert_eq!(attempts.len(), 3);
        assert_eq!(attempts[0].proxies.len(), 1);
        assert_eq!(attempts[1].proxies.len(), 1);
        assert!(attempts[2].proxies.is_empty());
        assert!(attempts[2].direct_fallback);
    }
}
