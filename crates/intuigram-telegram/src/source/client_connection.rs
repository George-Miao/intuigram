use super::*;

impl Client {
    /// Connects to a Telegram data center and generates fresh authorization
    /// material.
    pub async fn connect_new(
        dc_id: i32,
        endpoint: SocketAddr,
        credentials: ApplicationCredentials,
        route: Route,
    ) -> Result<(Self, Session)> {
        Self::connect_new_routed(dc_id, dc_id, endpoint, credentials, route).await
    }

    pub(super) async fn connect_new_media(
        dc_id: i32,
        endpoint: SocketAddr,
        credentials: ApplicationCredentials,
        route: Route,
    ) -> Result<(Self, Session)> {
        Self::connect_new_routed(dc_id, -dc_id, endpoint, credentials, route).await
    }

    async fn connect_new_routed(
        dc_id: i32,
        proxy_dc_id: i32,
        endpoint: SocketAddr,
        credentials: ApplicationCredentials,
        route: Route,
    ) -> Result<(Self, Session)> {
        let mut last_error = None;
        for attempt in route_attempts(&route) {
            let deadline = attempt.timeout;
            let connection = compio::time::timeout(
                deadline,
                Self::connect_new_attempt(
                    dc_id,
                    proxy_dc_id,
                    endpoint,
                    credentials.clone(),
                    attempt,
                    route.clone(),
                ),
            )
            .await;
            match connection {
                Ok(Ok(connected)) => return Ok(connected),
                Ok(Err(error)) => last_error = Some(error),
                Err(_) => last_error = Some(Error::RouteInitializationTimeout { endpoint }),
            }
        }
        Err(last_error.expect("every route policy has at least one connection attempt"))
    }

    async fn connect_new_attempt(
        dc_id: i32,
        proxy_dc_id: i32,
        endpoint: SocketAddr,
        credentials: ApplicationCredentials,
        attempt: Route,
        route: Route,
    ) -> Result<(Self, Session)> {
        let mut transport = connect_route(endpoint, proxy_dc_id, &attempt)
            .await
            .context(ConnectSnafu { endpoint })?;
        let material = generate_auth_key(&mut transport)
            .await
            .context(GenerateKeySnafu)?;
        let session = Session {
            dc_id,
            endpoint,
            auth_key: material.auth_key,
            time_offset: material.time_offset,
            first_salt: material.first_salt,
        };
        let mut client = Self {
            connection: Connection::Login(Box::new(EncryptedConnection::from_boxed(
                transport, &material,
            ))),
            dc_id,
            route,
            credentials,
            password: None,
            identity: None,
            peers: PeerDirectory::default(),
            names: HashMap::new(),
            channel_pts: HashMap::new(),
            data_centers: HashMap::new(),
            venue_search_username: None,
            venue_search_bot: None,
        };
        client.initialize().await?;
        Ok((client, session))
    }

    /// Verifies a complete MTProto handshake and Telegram API initialization.
    pub async fn test_connection(
        dc_id: i32,
        endpoint: SocketAddr,
        credentials: ApplicationCredentials,
        route: Route,
    ) -> Result<()> {
        Self::connect_new(dc_id, endpoint, credentials, route)
            .await
            .map(|_| ())
    }

    /// Reconnects with authorization material loaded from Account storage.
    pub async fn connect_existing(
        credentials: ApplicationCredentials,
        session: &Session,
        identity: AuthorizedUser,
        route: Route,
    ) -> Result<Self> {
        Self::connect_with_session(credentials, session, Some(identity), route).await
    }

    /// Reconnects an incomplete login using authorization material saved in
    /// `.pending.db`.
    pub async fn connect_pending(
        credentials: ApplicationCredentials,
        session: &Session,
        route: Route,
    ) -> Result<Self> {
        Self::connect_with_session(credentials, session, None, route).await
    }

    pub(super) async fn connect_with_session(
        credentials: ApplicationCredentials,
        session: &Session,
        identity: Option<AuthorizedUser>,
        route: Route,
    ) -> Result<Self> {
        let mut last_error = None;
        for attempt in route_attempts(&route) {
            let deadline = attempt.timeout;
            let connection = compio::time::timeout(
                deadline,
                Self::connect_session_attempt(
                    credentials.clone(),
                    session,
                    identity.clone(),
                    attempt,
                    route.clone(),
                ),
            )
            .await;
            match connection {
                Ok(Ok(client)) => return Ok(client),
                Ok(Err(error)) => last_error = Some(error),
                Err(_) => {
                    last_error = Some(Error::RouteInitializationTimeout {
                        endpoint: session.endpoint,
                    });
                }
            }
        }
        Err(last_error.expect("every route policy has at least one connection attempt"))
    }

    async fn connect_session_attempt(
        credentials: ApplicationCredentials,
        session: &Session,
        identity: Option<AuthorizedUser>,
        attempt: Route,
        route: Route,
    ) -> Result<Self> {
        let endpoint = session.endpoint;
        let transport = connect_route(endpoint, session.dc_id, &attempt)
            .await
            .context(ConnectSnafu { endpoint })?;
        let material = AuthKeyMaterial {
            auth_key: session.auth_key(),
            time_offset: session.time_offset,
            first_salt: session.first_salt,
        };
        let mut client = Self {
            connection: Connection::Login(Box::new(EncryptedConnection::from_boxed(
                transport, &material,
            ))),
            dc_id: session.dc_id,
            route,
            credentials,
            password: None,
            identity,
            peers: PeerDirectory::default(),
            names: HashMap::new(),
            channel_pts: HashMap::new(),
            data_centers: HashMap::new(),
            venue_search_username: None,
            venue_search_bot: None,
        };
        client.initialize().await?;
        Ok(client)
    }

    /// Converts an authenticated sequential connection into a cloneable raw
    /// invocation endpoint and continuously driven update stream.
    #[must_use]
    pub fn into_live(self, capacity: NonZeroUsize) -> (Self, LiveUpdates, PeerDirectory) {
        let Self {
            connection,
            dc_id,
            route,
            credentials,
            password,
            identity,
            peers,
            names,
            channel_pts,
            data_centers,
            venue_search_username,
            venue_search_bot,
        } = self;
        let directory = peers.clone();
        let Connection::Login(connection) = connection else {
            unreachable!("a Telegram client enters live mode only once after login")
        };
        let (handle, updates, driver) = (*connection).into_driver(capacity);
        (
            Self {
                connection: Connection::Live(handle),
                dc_id,
                route,
                credentials,
                password,
                identity,
                peers,
                names: names.clone(),
                channel_pts,
                data_centers,
                venue_search_username,
                venue_search_bot,
            },
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
