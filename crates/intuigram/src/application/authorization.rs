pub(super) async fn resume_account(
    credentials: ApplicationCredentials,
    layout: &StoreLayout,
    account: &AccountRecord,
    storage: AdapterStorage,
) -> Result<(
    Backend,
    BackendEvents,
    intuigram_telegram::PeerDirectory,
    Bootstrap,
)> {
    let database = AccountDatabase::open_with_cipher(layout, account.id, storage.cipher.clone())
        .context(AccountDatabaseSnafu)?;
    let cached = database.cached_account().context(AccountDatabaseSnafu)?;
    let cached_cursors = cached.cursors.clone();
    let stored =
        database
            .session()
            .context(AccountDatabaseSnafu)?
            .context(MissingSessionSnafu {
                account: account.id,
            })?;
    let session = telegram_session(&stored)?;
    let identity = AuthorizedUser {
        id: account.id.get(),
        display_name: account.display_name.clone(),
        username: None,
    };
    let mut client =
        Client::connect_existing(credentials, &session, identity, storage.route.clone())
            .await
            .context(TelegramSnafu)?;
    let mut bootstrap = client.bootstrap(100).await.context(TelegramSnafu)?;
    let cached = cached_bootstrap(
        account.display_name.clone(),
        account.notification_identity.clone(),
        cached,
    );
    bootstrap.drafts = cached.drafts;
    bootstrap.histories = cached.histories;
    bootstrap.offline_chats = cached.offline_chats;
    let current_cursors = client
        .synchronization_cursors()
        .await
        .context(TelegramSnafu)?
        .into_iter()
        .map(store_cursor)
        .collect::<Vec<_>>();
    let mut cursors = cached_cursors
        .into_iter()
        .map(|cursor| (cursor.scope.clone(), cursor))
        .collect::<BTreeMap<_, _>>();
    for cursor in current_cursors {
        cursors.insert(cursor.scope.clone(), cursor);
    }
    let cursors = cursors.into_values().collect::<Vec<_>>();
    database
        .commit_sync(bootstrap_sync_batch(&bootstrap, cursors.clone()))
        .context(AccountDatabaseSnafu)?;
    let store = database.store();
    let live_capacity = NonZeroUsize::new(EFFECT_CAPACITY)
        .expect("the constant MTProto request capacity is positive");
    let (client, updates, peers) = client.into_live(live_capacity);
    Ok((
        Backend {
            client: Box::new(client),
            _database: database,
            store: store.clone(),
            attachments: AttachmentStore::default(),
            media_library: MediaLibraryStore::default(),
            downloads: intuigram_media::DownloadDirectory::new(storage.downloads.clone()),
            media_cache: storage.for_account(account.id),
            downloaded: DownloadStore::default(),
        },
        BackendEvents {
            updates,
            committer: UpdateCommitter::new(
                store,
                cursors,
                bootstrap.chats.iter().map(|chat| chat.id),
            ),
            pending: None,
            pending_submission: None,
            queued_submission: None,
            pending_events: VecDeque::new(),
            submitted_updates: SubmittedUpdates::default(),
            stopped: false,
        },
        peers,
        bootstrap,
    ))
}

pub(super) async fn authorize_new_account(
    credentials: &ApplicationCredentials,
    config: &Config,
    layout: &StoreLayout,
    global: &GlobalDatabase,
    cipher: AccountCipher,
) -> Result<AccountId> {
    let route = telegram_route(config)?;
    let pending = AccountDatabase::begin_login_with_cipher(layout, cipher.clone())
        .context(AccountDatabaseSnafu)?;
    let (client, session) = if let Some(stored) = pending.session().context(AccountDatabaseSnafu)? {
        let session = telegram_session(&stored)?;
        match Client::connect_pending(credentials.clone(), &session, route.clone()).await {
            Ok(client) => (client, session),
            Err(error) if error.is_test_data_center() => {
                let connected = Client::connect_new(
                    PRIMARY_DC_ID,
                    PRIMARY_DC_ENDPOINT,
                    credentials.clone(),
                    route.clone(),
                )
                .await
                .context(TelegramSnafu)?;
                pending
                    .save_session(store_session(&connected.1))
                    .context(AccountDatabaseSnafu)?;
                connected
            }
            Err(source) => return Err(Error::Telegram { source }),
        }
    } else {
        let (client, session) = Client::connect_new(
            PRIMARY_DC_ID,
            PRIMARY_DC_ENDPOINT,
            credentials.clone(),
            route,
        )
        .await
        .context(TelegramSnafu)?;
        pending
            .save_session(store_session(&session))
            .context(AccountDatabaseSnafu)?;
        (client, session)
    };
    let (mut client, session, user) =
        match authorize_with_qr(credentials, &pending, client, session).await? {
            QrAuthorization::Authorized(authorized) => *authorized,
            QrAuthorization::PhoneLogin(login) => {
                let (mut client, mut session) = *login;
                let mut phone_number = config
                    .telegram
                    .phone_number
                    .as_deref()
                    .unwrap_or_default()
                    .to_owned();
                let mut error = None;
                let code_request = loop {
                    if config.telegram.phone_number.is_none() || error.is_some() {
                        phone_number = prompt_phone_number(&phone_number, error.as_deref())?;
                    }
                    match request_code_with_migration(
                        credentials,
                        &pending,
                        &mut client,
                        &mut session,
                        &phone_number,
                    )
                    .await
                    {
                        Ok(request) => break request,
                        Err(Error::Telegram { source }) if !source.is_connection_failure() => {
                            error = Some(source.to_string());
                        }
                        Err(error) => return Err(error),
                    }
                };
                let user = match code_request {
                    CodeRequest::AlreadyAuthorized(user) => user,
                    CodeRequest::Sent(token) => {
                        sign_in_with_delivered_code(&mut client, token).await?
                    }
                };
                (client, session, user)
            }
        };
    let account_id = AccountId::new(user.id).context(InvalidAccountIdSnafu { value: user.id })?;
    pending
        .save_session(store_session(&session))
        .context(AccountDatabaseSnafu)?;
    let database = pending
        .finish_login(layout, account_id)
        .context(AccountDatabaseSnafu)?;
    global
        .register(AccountRecord {
            id: account_id,
            display_name: user.display_name.clone(),
            active: true,
            notification_identity: format!("telegram:{}", account_id.get()),
        })
        .context(UpdateAccountRegistrySnafu)?;
    let bootstrap = client.bootstrap(100).await.context(TelegramSnafu)?;
    let cursors = client
        .synchronization_cursors()
        .await
        .context(TelegramSnafu)?
        .into_iter()
        .map(store_cursor)
        .collect::<Vec<_>>();
    database
        .commit_sync(bootstrap_sync_batch(&bootstrap, cursors.clone()))
        .context(AccountDatabaseSnafu)?;
    drop(client);
    drop(database);
    Ok(account_id)
}

pub(super) async fn authorize_with_qr(
    credentials: &ApplicationCredentials,
    pending: &AccountDatabase,
    mut client: Client,
    mut session: Session,
) -> Result<QrAuthorization> {
    let mut terminal = QrLoginUi::enter().context(TerminalSnafu)?;
    let mut state = client.export_qr_login().await.context(TelegramSnafu)?;
    loop {
        match state {
            QrLogin::Pending(token) => loop {
                let expires_in = seconds_until(token.expires_at(), session.time_offset);
                if expires_in == 0 {
                    state = client.export_qr_login().await.context(TelegramSnafu)?;
                    break;
                }
                terminal
                    .draw(token.uri(), expires_in)
                    .context(TerminalSnafu)?;
                match terminal
                    .poll_action(Duration::ZERO)
                    .context(TerminalSnafu)?
                {
                    QrLoginAction::PhoneLogin => {
                        return Ok(QrAuthorization::PhoneLogin(Box::new((client, session))));
                    }
                    QrLoginAction::Cancel => return LoginCancelledSnafu.fail(),
                    QrLoginAction::None | QrLoginAction::Redraw => {}
                }
                if client.poll_qr_login().await.context(TelegramSnafu)? {
                    state = client.export_qr_login().await.context(TelegramSnafu)?;
                    break;
                }
            },
            QrLogin::Migrate(migration) => {
                let dc_id = migration.dc_id();
                let endpoint = client
                    .data_center_endpoint(dc_id)
                    .context(MissingDataCenterSnafu { dc_id })?;
                let route = client.connection_route();
                (client, session) =
                    Client::connect_new(dc_id, endpoint, credentials.clone(), route)
                        .await
                        .context(TelegramSnafu)?;
                pending
                    .save_session(store_session(&session))
                    .context(AccountDatabaseSnafu)?;
                state = client
                    .import_qr_login(migration)
                    .await
                    .context(TelegramSnafu)?;
            }
            QrLogin::PasswordRequired(password) => {
                drop(terminal);
                let user = sign_in_with_password(&mut client, password).await?;
                return Ok(QrAuthorization::Authorized(Box::new((
                    client, session, user,
                ))));
            }
            QrLogin::Authorized(user) => {
                return Ok(QrAuthorization::Authorized(Box::new((
                    client, session, user,
                ))));
            }
        }
    }
}
use super::*;
