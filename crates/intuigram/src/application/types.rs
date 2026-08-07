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

#[derive(Default)]
pub(super) struct AttachmentStore {
    pub(super) next_id: u64,
    pub(super) payloads: HashMap<AttachmentId, AttachmentPayload>,
}

#[derive(Clone)]
pub(super) enum AttachmentPayload {
    Image { mime_type: String, bytes: Vec<u8> },
    File { path: PathBuf, kind: AttachmentKind },
}

#[derive(Default)]
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
    pub(super) pending_events: VecDeque<AdapterEvent>,
    pub(super) submitted_updates: VecDeque<intuigram_telegram::LiveEvent>,
}

pub(super) enum QrAuthorization {
    Authorized(Box<(Client, Session, AuthorizedUser)>),
    PhoneLogin(Box<(Client, Session)>),
}
