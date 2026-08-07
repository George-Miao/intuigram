impl Client {
    /// Connects to a Telegram data center and generates fresh authorization
    /// material.
    pub async fn connect_new(
        dc_id: i32,
        endpoint: SocketAddr,
        credentials: ApplicationCredentials,
    ) -> Result<(Self, Session)> {
        let transport = AbridgedConnection::connect(endpoint)
            .await
            .context(ConnectSnafu { endpoint })?;
        let mut transport = BoxedTransport::new(transport);
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
            credentials,
            password: None,
            identity: None,
            peers: PeerDirectory::default(),
            names: HashMap::new(),
            channel_pts: HashMap::new(),
            data_centers: HashMap::new(),
        };
        client.initialize().await?;
        Ok((client, session))
    }

    /// Reconnects with authorization material loaded from Account storage.
    pub async fn connect_existing(
        credentials: ApplicationCredentials,
        session: &Session,
        identity: AuthorizedUser,
    ) -> Result<Self> {
        Self::connect_with_session(credentials, session, Some(identity)).await
    }

    /// Reconnects an incomplete login using authorization material saved in
    /// `.pending.db`.
    pub async fn connect_pending(
        credentials: ApplicationCredentials,
        session: &Session,
    ) -> Result<Self> {
        Self::connect_with_session(credentials, session, None).await
    }

    pub(super) async fn connect_with_session(
        credentials: ApplicationCredentials,
        session: &Session,
        identity: Option<AuthorizedUser>,
    ) -> Result<Self> {
        let endpoint = session.endpoint;
        let transport = AbridgedConnection::connect(endpoint)
            .await
            .context(ConnectSnafu { endpoint })?;
        let material = AuthKeyMaterial {
            auth_key: session.auth_key(),
            time_offset: session.time_offset,
            first_salt: session.first_salt,
        };
        let mut client = Self {
            connection: Connection::Login(Box::new(EncryptedConnection::new(transport, &material))),
            dc_id: session.dc_id,
            credentials,
            password: None,
            identity,
            peers: PeerDirectory::default(),
            names: HashMap::new(),
            channel_pts: HashMap::new(),
            data_centers: HashMap::new(),
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
            credentials,
            password,
            identity,
            peers,
            names,
            channel_pts,
            data_centers,
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
                credentials,
                password,
                identity,
                peers,
                names: names.clone(),
                channel_pts,
                data_centers,
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
}
use super::*;
