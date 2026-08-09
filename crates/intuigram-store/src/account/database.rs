/// A database containing one Telegram account's durable state.
pub struct AccountDatabase {
    commands: SyncSender<Command>,
    worker: Option<JoinHandle<()>>,
    cipher: AccountCipher,
}

impl AccountDatabase {
    /// Opens an Account or returns a non-destructive recovery description when
    /// its existing database cannot be trusted.
    pub fn open_recoverable(
        layout: &StoreLayout,
        account: AccountId,
    ) -> Result<crate::AccountOpen> {
        Self::open_recoverable_with_cipher(layout, account, AccountCipher::plaintext())
    }

    /// Opens an Account with its configured Local Lock key.
    pub fn open_recoverable_with_cipher(
        layout: &StoreLayout,
        account: AccountId,
        cipher: AccountCipher,
    ) -> Result<crate::AccountOpen> {
        let path = layout.account_database(account);
        if !path.is_file() {
            return MissingDatabaseSnafu { path }.fail();
        }
        Ok(
            match Self::open_with_cipher(layout, account, cipher.clone()) {
                Ok(database) => crate::AccountOpen::Ready(database),
                Err(cause) => crate::AccountOpen::Recovery(Box::new(
                    crate::AccountRecovery::inspect(layout, account, cipher, cause),
                )),
            },
        )
    }

    /// Returns a cloneable nonblocking endpoint for runtime adapter tasks.
    #[must_use]
    pub fn store(&self) -> AccountStore {
        AccountStore {
            commands: self.commands.clone(),
        }
    }

    /// Creates and migrates the database used during login.
    pub fn begin_login(layout: &StoreLayout) -> Result<Self> {
        Self::begin_login_with_cipher(layout, AccountCipher::plaintext())
    }

    /// Creates and migrates an encrypted pending-login database.
    pub fn begin_login_with_cipher(layout: &StoreLayout, cipher: AccountCipher) -> Result<Self> {
        Self::spawn(layout.pending_database(), true, cipher)
    }

    /// Stores the authorized Telegram user ID and atomically promotes the
    /// database.
    pub fn finish_login(mut self, layout: &StoreLayout, account: AccountId) -> Result<Self> {
        let target = layout.account_database(account);
        self.write_account_id(account)?;
        self.stop()?;
        let pending = layout.pending_database();
        if let Err(source) = promote_without_replace(&pending, &target) {
            if source.kind() == std::io::ErrorKind::AlreadyExists {
                return AccountAlreadyExistsSnafu { path: target }.fail();
            }
            return Err(Error::PromoteDatabase {
                from: pending,
                to: target,
                source,
            });
        }
        Self::spawn(target, false, self.cipher.clone())
    }

    /// Opens a previously authorized account database.
    pub fn open(layout: &StoreLayout, account: AccountId) -> Result<Self> {
        Self::open_with_cipher(layout, account, AccountCipher::plaintext())
    }

    /// Opens a previously authorized Account with its Local Lock key.
    pub fn open_with_cipher(
        layout: &StoreLayout,
        account: AccountId,
        cipher: AccountCipher,
    ) -> Result<Self> {
        let path = layout.account_database(account);
        if !path.is_file() {
            return MissingDatabaseSnafu { path }.fail();
        }
        let database = Self::spawn(path, false, cipher)?;
        let actual = database.account_id()?;
        if actual != Some(account) {
            return IdentityMismatchSnafu {
                expected: account,
                actual,
            }
            .fail();
        }
        Ok(database)
    }

    /// Returns the Telegram user ID persisted in this database.
    pub fn account_id(&self) -> Result<Option<AccountId>> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::ReadIdentity { reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Returns the current durable Telegram authorization, when present.
    pub fn session(&self) -> Result<Option<SessionMaterial>> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::ReadSession { reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Persists a Telegram authorization before it can be used by the UI.
    pub fn save_session(&self, session: SessionMaterial) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::WriteSession {
                session: Box::new(session),
                reply,
            })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Atomically upserts normalized records and advances their cursor.
    pub fn commit_sync(&self, batch: SyncBatch) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::CommitSync {
                batch: Box::new(batch),
                reply,
            })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Loads the complete immediately renderable synchronized cache.
    pub fn cached_account(&self) -> Result<CachedAccount> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::LoadCache { reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Persists a Draft before callers report it as saved.
    pub fn save_draft(&self, draft: StoredDraft) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::SaveDraft { draft, reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Persists the selected Folder and Chat before shutdown.
    pub fn save_selection(&self, selection: StoredSelection) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::SaveSelection { selection, reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Persists whether one Chat's original media is protected from eviction.
    pub fn set_chat_media_offline(&self, chat_id: i64, keep: bool) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::SetChatMediaOffline {
                chat_id,
                keep,
                reply,
            })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Atomically replaces one Chat's complete ordered Topic projection.
    pub fn save_topics(&self, chat: i64, topics: Vec<StoredTopic>) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::SaveTopics {
                chat,
                topics,
                reply,
            })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Atomically replaces one Saved Messages Chat's per-origin dialog
    /// projection.
    pub fn save_saved_dialogs(&self, chat: i64, dialogs: Vec<StoredSavedDialog>) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::SaveSavedDialogs {
                chat,
                dialogs,
                reply,
            })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    fn spawn(path: PathBuf, create: bool, cipher: AccountCipher) -> Result<Self> {
        prepare_data_directory(&path)?;
        let (commands, requests) = mpsc::sync_channel(32);
        let (ready, initialized) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("intuigram-account-db".to_owned())
            .spawn({
                let cipher = cipher.clone();
                move || run_worker(&path, create, cipher, &requests, &ready)
            })
            .context(SpawnWorkerSnafu)?;
        initialized.recv().map_err(|_| Error::WorkerUnavailable)??;
        Ok(Self {
            commands,
            worker: Some(worker),
            cipher,
        })
    }

    fn write_account_id(&self, account: AccountId) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::WriteIdentity { account, reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    fn stop(&mut self) -> Result<()> {
        self.commands
            .send(Command::Shutdown)
            .map_err(|_| Error::WorkerUnavailable)?;
        self.worker
            .take()
            .ok_or(Error::WorkerUnavailable)?
            .join()
            .map_err(|_| Error::WorkerPanicked)
    }
}

impl Drop for AccountDatabase {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
use super::*;
