use super::*;

impl Client {
    /// Connects to the first available Telegram data-center endpoint and
    /// generates fresh authorization material.
    pub async fn connect_new(
        dc_id: i32,
        endpoints: &[SocketAddr],
        credentials: ApplicationCredentials,
        route: Route,
    ) -> Result<(Self, Session)> {
        Self::connect_new_routed(dc_id, dc_id, endpoints, credentials, route).await
    }

    pub(crate) async fn connect_new_media(
        dc_id: i32,
        endpoints: &[SocketAddr],
        credentials: ApplicationCredentials,
        route: Route,
    ) -> Result<(Self, Session)> {
        Self::connect_new_routed(dc_id, -dc_id, endpoints, credentials, route).await
    }

    async fn connect_new_routed(
        dc_id: i32,
        proxy_dc_id: i32,
        endpoints: &[SocketAddr],
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
                    endpoints,
                    credentials.clone(),
                    attempt,
                    route.clone(),
                ),
            )
            .await;
            match connection {
                Ok(Ok(connected)) => return Ok(connected),
                Ok(Err(error)) => last_error = Some(error),
                Err(_) => {
                    last_error = Some(Error::RouteInitializationTimeout {
                        endpoints: endpoints.to_vec(),
                    });
                }
            }
        }
        Err(last_error.expect("every route policy has at least one connection attempt"))
    }

    async fn connect_new_attempt(
        dc_id: i32,
        proxy_dc_id: i32,
        endpoints: &[SocketAddr],
        credentials: ApplicationCredentials,
        attempt: Route,
        route: Route,
    ) -> Result<(Self, Session)> {
        let (mut transport, endpoint) = connect_route(endpoints, proxy_dc_id, &attempt)
            .await
            .with_context(|_| ConnectSnafu {
                endpoints: endpoints.to_vec(),
            })?;
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
            session: session.clone(),
            dc_id,
            route,
            credentials,
            password: None,
            identity: None,
            peers: PeerDirectory::default(),
            names: HashMap::new(),
            channel_pts: HashMap::new(),
            data_centers: HashMap::new(),
            media_data_centers: HashMap::new(),
            cdn_data_centers: HashMap::new(),
            media_sessions: None,
            venue_search_username: None,
            venue_search_bot: None,
        };
        client.initialize().await?;
        Ok((client, session))
    }

    /// Verifies a complete MTProto handshake and Telegram API initialization
    /// against the first available endpoint.
    pub async fn test_connection(
        dc_id: i32,
        endpoints: &[SocketAddr],
        credentials: ApplicationCredentials,
        route: Route,
    ) -> Result<()> {
        Self::connect_new(dc_id, endpoints, credentials, route)
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

    pub(crate) async fn connect_with_session(
        credentials: ApplicationCredentials,
        session: &Session,
        identity: Option<AuthorizedUser>,
        route: Route,
    ) -> Result<Self> {
        Self::connect_with_session_endpoints(
            credentials,
            session,
            std::slice::from_ref(&session.endpoint),
            identity,
            route,
        )
        .await
    }

    pub(crate) async fn connect_with_session_endpoints(
        credentials: ApplicationCredentials,
        session: &Session,
        endpoints: &[SocketAddr],
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
                    endpoints,
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
                        endpoints: endpoints.to_vec(),
                    });
                }
            }
        }
        Err(last_error.expect("every route policy has at least one connection attempt"))
    }

    async fn connect_session_attempt(
        credentials: ApplicationCredentials,
        session: &Session,
        endpoints: &[SocketAddr],
        identity: Option<AuthorizedUser>,
        attempt: Route,
        route: Route,
    ) -> Result<Self> {
        let (transport, endpoint) = connect_route(endpoints, session.dc_id, &attempt)
            .await
            .with_context(|_| ConnectSnafu {
                endpoints: endpoints.to_vec(),
            })?;
        let material = AuthKeyMaterial {
            auth_key: session.auth_key(),
            time_offset: session.time_offset,
            first_salt: session.first_salt,
        };
        let mut session = session.clone();
        session.endpoint = endpoint;
        let dc_id = session.dc_id;
        let mut client = Self {
            connection: Connection::Login(Box::new(EncryptedConnection::from_boxed(
                transport, &material,
            ))),
            session,
            dc_id,
            route,
            credentials,
            password: None,
            identity,
            peers: PeerDirectory::default(),
            names: HashMap::new(),
            channel_pts: HashMap::new(),
            data_centers: HashMap::new(),
            media_data_centers: HashMap::new(),
            cdn_data_centers: HashMap::new(),
            media_sessions: None,
            venue_search_username: None,
            venue_search_bot: None,
        };
        client.initialize().await?;
        Ok(client)
    }
}
