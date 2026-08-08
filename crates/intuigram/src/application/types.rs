use super::*;

#[derive(Default)]
pub(super) struct Arguments {
    pub(super) config: Option<PathBuf>,
    pub(super) data: Option<PathBuf>,
    pub(super) cache: Option<PathBuf>,
    pub(super) downloads: Option<PathBuf>,
    pub(super) maintenance: Option<Maintenance>,
    pub(super) account: Option<AccountId>,
    pub(super) add_account: bool,
    pub(super) list_accounts: bool,
    pub(super) test_connection: bool,
    pub(super) help: bool,
}

#[derive(Clone)]
pub(super) enum Maintenance {
    MediaUsage(AccountId),
    ClearMedia(AccountId),
    ClearAccount(AccountId),
    Logout(AccountId),
    Folder(AccountId, FolderMaintenance),
    RichMedia(AccountId, RichMediaMaintenance),
    Scheduled(AccountId, ScheduledMaintenance),
}

#[derive(Clone)]
pub(super) enum FolderMaintenance {
    Create { title: String, rules: FolderRules },
    Rename { folder: i32, title: String },
    Reorder { folder: i32, position: usize },
    Share { folder: i32 },
    Delete { folder: i32 },
    Rules { folder: i32, rules: FolderRules },
}

#[derive(Clone)]
pub(super) enum RichMediaMaintenance {
    Browse {
        kind: MediaLibraryKind,
        query: String,
    },
    SendLibrary {
        chat: ChatId,
        kind: MediaLibraryKind,
        index: usize,
        query: String,
    },
    SendFile {
        chat: ChatId,
        kind: UploadKind,
        path: PathBuf,
    },
    Record {
        chat: ChatId,
        kind: UploadKind,
        seconds: u32,
        device: String,
    },
    Contact {
        chat: ChatId,
        phone: String,
        first_name: String,
        last_name: String,
    },
}

#[derive(Clone)]
pub(super) enum ScheduledMaintenance {
    Create {
        chat: ChatId,
        delivery: ScheduledDelivery,
        text: String,
    },
    List {
        chat: ChatId,
    },
    Edit {
        chat: ChatId,
        message: i32,
        text: String,
    },
    Reschedule {
        chat: ChatId,
        message: i32,
        delivery: ScheduledDelivery,
    },
    Delete {
        chat: ChatId,
        message: i32,
    },
    SendNow {
        chat: ChatId,
        message: i32,
    },
}

pub(super) struct Backend {
    pub(super) client: Box<Client>,
    pub(super) _database: AccountDatabase,
    pub(super) store: AccountStore,
    pub(super) next_local_message_id: i64,
    pub(super) attachments: AttachmentStore,
    pub(super) media_library: MediaLibraryStore,
    pub(super) downloads: intuigram_media::DownloadDirectory,
    pub(super) media_cache: intuigram_media::MediaCache,
    pub(super) downloaded: DownloadStore,
}

#[derive(Default)]
pub(super) struct RetainedBackend {
    pub(super) attachments: AttachmentStore,
    pub(super) media_library: MediaLibraryStore,
    pub(super) downloaded: DownloadStore,
}

#[derive(Clone)]
pub(super) struct AdapterStorage {
    pub(super) downloads: PathBuf,
    pub(super) cache_root: PathBuf,
    pub(super) cache_limit: u64,
    pub(super) cipher: AccountCipher,
    pub(super) route: compio_mtproto::Route,
}

impl AdapterStorage {
    pub(super) fn for_account(&self, account: AccountId) -> intuigram_media::MediaCache {
        intuigram_media::MediaCache::new(
            self.cache_root.join(account.get().to_string()),
            self.cache_limit,
        )
    }
}

#[derive(Clone, Default)]
pub(super) struct AttachmentStore {
    pub(super) next_id: u64,
    pub(super) payloads: HashMap<AttachmentId, AttachmentPayload>,
}

#[derive(Clone)]
pub(super) enum AttachmentPayload {
    Image {
        mime_type: String,
        bytes: Vec<u8>,
    },
    File {
        path: PathBuf,
        kind: AttachmentKind,
    },
    PreparedFile {
        name: String,
        mime_type: String,
        bytes: Vec<u8>,
        kind: AttachmentKind,
    },
}

pub(super) struct PreparedRichMedia {
    pub(super) name: String,
    pub(super) mime_type: String,
    pub(super) bytes: Vec<u8>,
    pub(super) kind: RichMediaUploadKind,
}

#[derive(Clone, Default)]
pub(super) struct DownloadStore {
    pub(super) next_id: u64,
    pub(super) paths: HashMap<DownloadId, PathBuf>,
}

#[derive(Default)]
pub(super) struct MediaLibraryStore {
    pub(super) next_id: u64,
    pub(super) entries: HashMap<RichMediaItemId, MediaLibraryEntry>,
}

impl MediaLibraryStore {
    pub(super) fn register(&mut self, entries: Vec<MediaLibraryEntry>) -> Vec<RichMediaItemView> {
        entries
            .into_iter()
            .map(|entry| {
                self.next_id = self.next_id.saturating_add(1);
                let id = RichMediaItemId(self.next_id);
                let view = RichMediaItemView {
                    id,
                    label: entry.label.clone(),
                };
                self.entries.insert(id, entry);
                view
            })
            .collect()
    }

    pub(super) fn merge(&mut self, mut other: Self) {
        self.next_id = self.next_id.max(other.next_id);
        self.entries.extend(other.entries.drain());
    }
}

impl DownloadStore {
    pub(super) fn register(&mut self, path: PathBuf) -> DownloadId {
        self.next_id = self.next_id.saturating_add(1);
        let id = DownloadId(self.next_id);
        self.paths.insert(id, path);
        id
    }

    pub(super) fn merge(&mut self, mut other: Self) {
        self.next_id = self.next_id.max(other.next_id);
        self.paths.extend(other.paths.drain());
    }
}

impl AttachmentStore {
    pub(super) fn register(&mut self, payload: AttachmentPayload) -> AttachmentId {
        self.next_id = self.next_id.saturating_add(1);
        let id = AttachmentId(self.next_id);
        self.payloads.insert(id, payload);
        id
    }

    pub(super) fn merge(&mut self, mut other: Self) {
        self.next_id = self.next_id.max(other.next_id);
        self.payloads.extend(other.payloads.drain());
    }
}

pub(super) struct BackendEvents {
    pub(super) updates: LiveUpdates,
    pub(super) committer: UpdateCommitter,
    pub(super) pending: Option<UpdateCommit>,
    pub(super) pending_submission: Option<SubmissionCompletion>,
    pub(super) queued_submission: Option<QueuedSubmission>,
    pub(super) pending_events: VecDeque<AdapterEvent>,
    pub(super) submitted_updates: SubmittedUpdates,
    pub(super) stopped: bool,
}

pub(super) type SubmissionResult = std::result::Result<(), Box<Error>>;

#[derive(Clone)]
pub(super) struct SubmissionCompletion {
    state: std::rc::Rc<std::cell::RefCell<SubmissionState>>,
}

pub(super) struct SubmissionReceipt {
    state: std::rc::Rc<std::cell::RefCell<SubmissionState>>,
}

#[derive(Default)]
struct SubmissionState {
    result: Option<SubmissionResult>,
    waker: Option<std::task::Waker>,
}

#[derive(Clone, Default)]
pub(super) struct SubmittedUpdates {
    inner: std::rc::Rc<std::cell::RefCell<SubmittedUpdateState>>,
}

#[derive(Default)]
struct SubmittedUpdateState {
    updates: VecDeque<SubmittedUpdate>,
    waker: Option<std::task::Waker>,
    closed: bool,
}

pub(super) struct SubmittedUpdate {
    pub(super) update: intuigram_telegram::LiveEvent,
    pub(super) committed: SubmissionCompletion,
}

pub(super) struct QueuedSubmission {
    pub(super) submission: SubmittedUpdate,
    pub(super) preceding_live_updates: usize,
}

impl QueuedSubmission {
    pub(super) const fn is_ready(&self) -> bool {
        self.preceding_live_updates == 0
    }

    pub(super) fn observe_live_update(&mut self) {
        self.preceding_live_updates = self.preceding_live_updates.saturating_sub(1);
    }
}

impl SubmissionCompletion {
    pub(super) fn complete(self, result: SubmissionResult) {
        let mut state = self.state.borrow_mut();
        state.result = Some(result);
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

impl Future for SubmissionReceipt {
    type Output = SubmissionResult;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.borrow_mut();
        if let Some(result) = state.result.take() {
            return Poll::Ready(result);
        }
        if state
            .waker
            .as_ref()
            .is_none_or(|waker| !waker.will_wake(cx.waker()))
        {
            state.waker = Some(cx.waker().clone());
        }
        Poll::Pending
    }
}

impl SubmittedUpdates {
    pub(super) fn push(&self, update: intuigram_telegram::LiveEvent) -> SubmissionReceipt {
        let state = std::rc::Rc::new(std::cell::RefCell::new(SubmissionState::default()));
        let committed = SubmissionCompletion {
            state: std::rc::Rc::clone(&state),
        };
        let receipt = SubmissionReceipt { state };
        let mut state = self.inner.borrow_mut();
        if state.closed {
            committed.complete(Err(Box::new(Error::TelegramActorCancelled)));
            return receipt;
        }
        state
            .updates
            .push_back(SubmittedUpdate { update, committed });
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
        receipt
    }

    pub(super) fn poll_pop(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<SubmittedUpdate>> {
        let mut state = self.inner.borrow_mut();
        match state.updates.pop_front() {
            Some(update) => Poll::Ready(Some(update)),
            None => {
                if state
                    .waker
                    .as_ref()
                    .is_none_or(|waker| !waker.will_wake(cx.waker()))
                {
                    state.waker = Some(cx.waker().clone());
                }
                Poll::Pending
            }
        }
    }

    pub(super) fn close(&self) {
        let mut state = self.inner.borrow_mut();
        state.closed = true;
        for update in state.updates.drain(..) {
            update
                .committed
                .complete(Err(Box::new(Error::TelegramActorCancelled)));
        }
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

impl Drop for BackendEvents {
    fn drop(&mut self) {
        self.submitted_updates.close();
    }
}

pub(super) enum QrAuthorization {
    Authorized(Box<(Client, Session, AuthorizedUser)>),
    PhoneLogin(Box<(Client, Session)>),
}

#[cfg(test)]
mod submitted_update_tests {
    use super::*;

    #[test]
    fn closing_the_driver_wakes_pending_commit_waiters() {
        let submitted = SubmittedUpdates::default();
        let committed = submitted.push(intuigram_telegram::LiveEvent {
            events: Vec::new(),
            cursors: Vec::new(),
            peers: intuigram_telegram::PeerDirectory::default(),
        });

        submitted.close();
        let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
        let result = runtime.block_on(committed);

        assert!(matches!(
            result,
            Err(error) if matches!(*error, Error::TelegramActorCancelled)
        ));
    }

    #[test]
    fn later_live_updates_do_not_extend_a_submission_barrier() {
        let submitted = SubmittedUpdates::default();
        let _receipt = submitted.push(intuigram_telegram::LiveEvent {
            events: Vec::new(),
            cursors: Vec::new(),
            peers: intuigram_telegram::PeerDirectory::default(),
        });
        let mut context = std::task::Context::from_waker(futures_util::task::noop_waker_ref());
        let Poll::Ready(Some(submission)) = submitted.poll_pop(&mut context) else {
            panic!("submission should be queued")
        };
        let mut barrier = QueuedSubmission {
            submission,
            preceding_live_updates: 2,
        };

        barrier.observe_live_update();
        assert!(!barrier.is_ready());
        barrier.observe_live_update();
        assert!(barrier.is_ready());
        barrier.observe_live_update();
        assert!(barrier.is_ready());
    }
}
