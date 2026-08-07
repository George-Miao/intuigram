pub(super) enum Command {
    ReadIdentity {
        reply: SyncSender<Result<Option<AccountId>>>,
    },
    WriteIdentity {
        account: AccountId,
        reply: SyncSender<Result<()>>,
    },
    ReadSession {
        reply: SyncSender<Result<Option<SessionMaterial>>>,
    },
    WriteSession {
        session: Box<SessionMaterial>,
        reply: SyncSender<Result<()>>,
    },
    CommitSync {
        batch: Box<SyncBatch>,
        reply: SyncSender<Result<()>>,
    },
    LoadCache {
        reply: SyncSender<Result<CachedAccount>>,
    },
    SaveDraft {
        draft: StoredDraft,
        reply: SyncSender<Result<()>>,
    },
    CommitSyncAsync {
        batch: Box<SyncBatch>,
        reply: AsyncReply<()>,
    },
    SaveDraftAsync {
        draft: StoredDraft,
        reply: AsyncReply<()>,
    },
    SaveMessagesAsync {
        messages: Vec<StoredMessage>,
        reply: AsyncReply<()>,
    },
    DeleteMessagesAsync {
        chat: Option<i64>,
        messages: Vec<i64>,
        reply: AsyncReply<()>,
    },
    Shutdown,
}

struct AsyncState<T> {
    result: Option<Result<T>>,
    waker: Option<Waker>,
}

pub(super) struct AsyncReply<T> {
    state: Arc<Mutex<AsyncState<T>>>,
}

impl<T> AsyncReply<T> {
    pub(super) fn finish(self, result: Result<T>) {
        let mut state = self
            .state
            .lock()
            .expect("database response mutex is not exposed to panicking user callbacks");
        state.result = Some(result);
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

/// Awaitable response from the dedicated blocking SQLite worker.
pub struct DatabaseRequest<T> {
    state: Arc<Mutex<AsyncState<T>>>,
}

impl<T> Future for DatabaseRequest<T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self
            .state
            .lock()
            .expect("database response mutex is not exposed to panicking user callbacks");
        if let Some(result) = state.result.take() {
            Poll::Ready(result)
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

fn async_response<T>() -> (AsyncReply<T>, DatabaseRequest<T>) {
    let state = Arc::new(Mutex::new(AsyncState {
        result: None,
        waker: None,
    }));
    (
        AsyncReply {
            state: Arc::clone(&state),
        },
        DatabaseRequest { state },
    )
}

/// Cloneable nonblocking request endpoint for an Account database worker.
#[derive(Clone)]
pub struct AccountStore {
    pub(super) commands: SyncSender<Command>,
}

impl AccountStore {
    /// Enqueues an atomic synchronized-cache commit without blocking the
    /// runtime.
    pub fn commit_sync(&self, batch: SyncBatch) -> Result<DatabaseRequest<()>> {
        let (reply, request) = async_response();
        self.commands
            .try_send(Command::CommitSyncAsync {
                batch: Box::new(batch),
                reply,
            })
            .map_err(map_try_send_error)?;
        Ok(request)
    }

    /// Enqueues a durable Draft commit without blocking the runtime.
    pub fn save_draft(&self, draft: StoredDraft) -> Result<DatabaseRequest<()>> {
        let (reply, request) = async_response();
        self.commands
            .try_send(Command::SaveDraftAsync { draft, reply })
            .map_err(map_try_send_error)?;
        Ok(request)
    }

    /// Enqueues an atomic normalized Message upsert without advancing a cursor.
    pub fn save_messages(&self, messages: Vec<StoredMessage>) -> Result<DatabaseRequest<()>> {
        let (reply, request) = async_response();
        self.commands
            .try_send(Command::SaveMessagesAsync { messages, reply })
            .map_err(map_try_send_error)?;
        Ok(request)
    }

    /// Enqueues durable Message removal without advancing a synchronization
    /// cursor.
    pub fn delete_messages(
        &self,
        chat: Option<i64>,
        messages: Vec<i64>,
    ) -> Result<DatabaseRequest<()>> {
        let (reply, request) = async_response();
        self.commands
            .try_send(Command::DeleteMessagesAsync {
                chat,
                messages,
                reply,
            })
            .map_err(map_try_send_error)?;
        Ok(request)
    }
}

fn map_try_send_error<T>(error: mpsc::TrySendError<T>) -> Error {
    match error {
        mpsc::TrySendError::Full(_) => Error::WorkerQueueFull,
        mpsc::TrySendError::Disconnected(_) => Error::WorkerUnavailable,
    }
}
use super::*;
