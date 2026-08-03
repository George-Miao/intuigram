//! Deterministic, single-owner application state for Intuigram.

use std::collections::HashMap;

/// Stable identifier for a Telegram chat.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChatId(pub i64);

/// Stable identifier for a Telegram message.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MessageId(pub i64);

/// Opaque attachment candidate owned by the composition adapter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AttachmentId(pub u64);

/// Current Telegram connection state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    /// The account is synchronized over a live connection.
    Connected,
    /// A connection attempt is in progress.
    Connecting,
    /// Automatic reconnection is waiting for its backoff deadline.
    ReconnectCooldown,
}

/// Current interaction target within the TUI hierarchy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Focus {
    /// Chat list.
    Chats,
    /// Active Chat transcript.
    Transcript,
    /// Message Draft editor.
    Composer,
    /// Context-sensitive search field.
    Search,
}

/// Scope selected when context-sensitive search opens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchScope {
    /// Search the active Chat.
    Chat,
    /// Search every synchronized Chat in the active Account.
    Account,
}

/// One Telegram Folder presented in the bottom Folder strip.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderView {
    /// Telegram Folder identifier.
    pub id: i32,
    /// Display name.
    pub title: String,
    /// Aggregate unread count.
    pub unread: u32,
}

/// Telegram cloud Chat category normalized away from TL constructors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatKind {
    /// The active Account's Saved Messages Chat.
    SavedMessages,

    /// A human Private Chat.
    Private,

    /// An ordinary bot Private Chat.
    Bot,

    /// A legacy basic group.
    BasicGroup,

    /// A modern group without gigagroup restrictions.
    Supergroup,

    /// A group where only administrators may post.
    Gigagroup,

    /// A broadcast Channel.
    Channel,

    /// Telegram exposed an identity that cannot currently be accessed.
    Inaccessible,
}

/// Dense summary of a synchronized Chat.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatView {
    /// Telegram Chat identifier.
    pub id: ChatId,
    /// Display name.
    pub title: String,
    /// Compact last-message preview.
    pub preview: String,
    /// Unread message count.
    pub unread: u32,
    /// Whether Telegram pins this Chat.
    pub pinned: bool,

    /// Normalized cloud Chat category.
    pub kind: ChatKind,

    /// Folder identifiers containing this Chat. `0` is All and `-1` Archive.
    pub folders: Vec<i32>,
}

/// Sender direction for transcript styling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageDirection {
    /// Message received from another peer.
    Incoming,
    /// Message sent by the active Account.
    Outgoing,
}

/// Delivery state kept separate from local durability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryState {
    /// Locally durable and waiting for Telegram acknowledgement.
    Pending,
    /// Telegram accepted the message.
    Sent,
    /// Telegram reports that the recipient read the message.
    Read,
    /// The send reached a terminal error.
    Failed,
}

/// Rich-text semantic recognized by Intuigram.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextEntityKind {
    /// Bold emphasis.
    Bold,

    /// Italic emphasis.
    Italic,

    /// Underlined text.
    Underline,

    /// Struck text.
    Strike,

    /// Inline code.
    Code,

    /// Preformatted code block with an optional language.
    Pre { language: Option<String> },

    /// Spoiler text.
    Spoiler,

    /// Ordinary URL present in the body.
    Url,

    /// Display text pointing at a separate URL.
    TextUrl { url: String },

    /// Mention, hashtag, cashtag, bot command, email, or phone token.
    Semantic,

    /// Custom emoji document.
    CustomEmoji { document_id: i64 },
}

/// One UTF-16-indexed Telegram rich-text entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextEntity {
    /// UTF-16 code-unit offset.
    pub offset: usize,

    /// UTF-16 code-unit length.
    pub length: usize,

    /// Entity semantic.
    pub kind: TextEntityKind,
}

/// Major media and specialized Message families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaKind {
    /// Photo.
    Photo,

    /// Video document.
    Video,

    /// Animated document.
    Animation,

    /// Sticker or custom emoji document.
    Sticker,

    /// Generic file.
    File,

    /// Music or other audio document.
    Audio,

    /// Voice note.
    Voice,

    /// Video note.
    VideoNote,

    /// Web page preview.
    LinkPreview,

    /// Poll or quiz.
    Poll,

    /// Contact card.
    Contact,

    /// Static location.
    Location,

    /// Venue.
    Venue,

    /// Dice result.
    Dice,

    /// Specialized content planned for interactive rendering.
    Specialized,

    /// Constructor not recognized by the current client.
    Unsupported,
}

/// Text-first Media Card data used by every renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaCard {
    /// Semantic family.
    pub kind: MediaKind,

    /// Short type or filename label.
    pub title: String,

    /// Useful metadata or caption fallback.
    pub description: String,

    /// Stable remote identifier used by download actions, when available.
    pub remote_id: Option<String>,
}

/// One aggregate reaction shown on a Message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReactionView {
    /// Emoji or semantic label.
    pub label: String,

    /// Aggregate reaction count.
    pub count: u32,

    /// Whether the active Account selected it.
    pub chosen: bool,
}

/// Rich and status metadata kept alongside a dense Message row.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MessageDetails {
    /// Telegram rich-text entities.
    pub entities: Vec<TextEntity>,

    /// Forward attribution, when present.
    pub forwarded_from: Option<String>,

    /// Aggregate reactions.
    pub reactions: Vec<ReactionView>,

    /// Telegram edit marker.
    pub edited: bool,

    /// Telegram pin marker.
    pub pinned: bool,

    /// View counter for Channels.
    pub views: Option<u32>,

    /// Forward counter.
    pub forwards: Option<u32>,

    /// Reply or comment counter.
    pub replies: Option<u32>,

    /// Media Card or Unsupported Content presentation.
    pub media: Option<MediaCard>,

    /// Service event description, when this is a service Message.
    pub service: Option<String>,

    /// Top Message ID for an ordinary Thread or Channel comments.
    pub thread_root: Option<MessageId>,
}

/// One dense transcript row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageView {
    /// Telegram Message identifier.
    pub id: MessageId,
    /// Sender display name.
    pub sender: String,
    /// Plain-text or semantic fallback body.
    pub body: String,
    /// Compact local-time label supplied by the adapter.
    pub timestamp: String,
    /// Incoming or outgoing presentation.
    pub direction: MessageDirection,
    /// Delivery/read state.
    pub delivery: DeliveryState,
    /// Message being replied to, when any.
    pub reply_to: Option<MessageId>,

    /// Rich content, counters, and semantic Media Card data.
    pub details: MessageDetails,
}

/// Current Draft state for the active Chat.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ComposerView {
    /// Draft text.
    pub text: String,
    /// Message targeted by a reply.
    pub reply_to: Option<MessageId>,

    /// Native clipboard or file attachment candidates.
    pub attachments: Vec<AttachmentView>,
}

/// Composer attachment category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentKind {
    /// Photo candidate sent with Telegram photo semantics.
    Photo,

    /// Generic file candidate.
    File,
}

/// Safe display data for an adapter-owned attachment candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentView {
    /// Opaque adapter identifier.
    pub id: AttachmentId,

    /// Semantic upload kind.
    pub kind: AttachmentKind,

    /// Filename or clipboard image label.
    pub name: String,
}

/// Active search query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchView {
    /// Search scope selected from the prior focus.
    pub scope: SearchScope,
    /// Query entered so far.
    pub query: String,
}

/// Durable Draft restored before an Account is presented.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftView {
    /// Owning Chat.
    pub chat: ChatId,

    /// Thread root, or `None` for the root Chat Draft.
    pub thread_root: Option<MessageId>,

    /// Unsent text.
    pub text: String,

    /// Replied-to Message, when any.
    pub reply_to: Option<MessageId>,
}

/// One immediately renderable cached root or Thread history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryView {
    /// Owning Chat.
    pub chat: ChatId,

    /// Thread root, or `None` for root Chat history.
    pub thread_root: Option<MessageId>,

    /// Chronological cached Messages.
    pub messages: Vec<MessageView>,
}

/// Context-sensitive actions shown by every user interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// Exit Intuigram cleanly.
    Quit,
    /// Open exhaustive context help.
    Help,
    /// Move the active item upward.
    MoveUp,
    /// Move the active item downward.
    MoveDown,
    /// Switch to the previous Folder from the Chat list.
    PreviousFolder,
    /// Switch to the next Folder from the Chat list.
    NextFolder,
    /// Enter the Active Chat with its Composer focused.
    Open,
    /// Focus the Draft editor.
    Compose,
    /// Send the current Draft.
    Send,
    /// Insert a line break into the current Draft.
    Newline,
    /// Query the native clipboard for text, images, or files.
    Paste,
    /// Reply to the Active Message.
    Reply,
    /// Open the Active Message's ordinary Thread or Channel comments.
    OpenThread,
    /// Target the previous Message, entering the Transcript from the Composer.
    TargetPreviousMessage,
    /// Target the next Message, returning to the Composer after the newest.
    TargetNextMessage,
    /// Search using the context selected by focus.
    Search,
    /// Cancel the active transient interaction.
    Cancel,
    /// Jump to the oldest loaded Message.
    JumpEarliest,
    /// Jump to the newest loaded Message.
    JumpLatest,
    /// Retry immediately during a reconnect cooldown.
    Reconnect,
}

/// User actions understood by the state owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Intent {
    /// Invoke an action resolved by the effective keymap.
    Action(Action),
    /// Insert text into the Draft or active search query.
    Insert(String),
    /// Remove the final character from the active text field.
    Backspace,
}

/// Initial synchronized data supplied by an adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bootstrap {
    /// Connectivity represented by this initial data source.
    pub connection: ConnectionState,

    /// Active Account display name.
    pub account_name: String,
    /// Synchronized Telegram Folders.
    pub folders: Vec<FolderView>,
    /// Chats in the active Folder.
    pub chats: Vec<ChatView>,
    /// Messages for the initially active Chat.
    pub messages: Vec<MessageView>,

    /// Durable root and Thread Drafts for cached Chats.
    pub drafts: Vec<DraftView>,

    /// Cached histories for immediate Chat switching.
    pub histories: Vec<HistoryView>,
}

/// Results reported by external adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterEvent {
    /// Initial synchronized data became available.
    Bootstrap(Bootstrap),
    /// Telegram connectivity changed.
    ConnectionChanged(ConnectionState),

    /// Automatic connection attempts entered cooldown after a failure.
    ConnectionFailed(String),

    /// A new or acknowledged Message belongs in a Chat history.
    MessageAdded {
        /// Chat that owns the Message.
        chat: ChatId,
        /// Newly available Message.
        message: Box<MessageView>,
    },
    /// A requested Chat history became available.
    ChatLoaded {
        /// Chat whose history was loaded.
        chat: ChatId,
        /// Chronological loaded history.
        messages: Vec<MessageView>,
    },
    /// A requested Thread history became available.
    ThreadLoaded {
        /// Parent Chat.
        chat: ChatId,

        /// Root Message of the Thread.
        root: MessageId,

        /// Chronological Thread history.
        messages: Vec<MessageView>,
    },
    /// Native clipboard content became available for a Composer.
    ClipboardReady {
        /// Chat whose Composer requested the paste.
        chat: ChatId,

        /// Thread Composer, when applicable.
        thread_root: Option<MessageId>,

        /// Text to insert.
        text: Option<String>,

        /// Adapter-owned attachment candidates.
        attachments: Vec<AttachmentView>,
    },
    /// Telegram acknowledged an optimistic local Message.
    MessageAcknowledged {
        /// Owning Chat.
        chat: ChatId,

        /// Pending local Message ID.
        local_id: MessageId,
    },

    /// A pending send reached a terminal failure.
    MessageFailed {
        /// Owning Chat.
        chat: ChatId,

        /// Pending local Message ID.
        local_id: MessageId,

        /// Thread Composer, when applicable.
        thread_root: Option<MessageId>,

        /// Draft text that must remain recoverable.
        text: String,

        /// User-facing semantic failure.
        reason: String,
    },
}

/// Ordered inputs to the state owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Input {
    /// An action from the active user interface.
    Intent(Intent),
    /// A result from an external adapter.
    Adapter(AdapterEvent),
}

/// Side effects requested from adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    /// Start a connection attempt immediately.
    Reconnect,
    /// Load recent history for the selected Chat.
    LoadChat {
        /// Chat selected by the user.
        chat: ChatId,
    },
    /// Load an ordinary Message Thread or Channel comments.
    LoadThread {
        /// Parent Chat.
        chat: ChatId,

        /// Thread root Message.
        root: MessageId,
    },
    /// Query the native clipboard without blocking terminal input.
    ReadClipboard {
        /// Chat whose Composer requested the paste.
        chat: ChatId,

        /// Thread Composer, when applicable.
        thread_root: Option<MessageId>,
    },
    /// Persist a changed Draft before any saved indication is emitted.
    SaveDraft {
        /// Owning Chat.
        chat: ChatId,

        /// Thread Composer, when applicable.
        thread_root: Option<MessageId>,

        /// Complete Draft text.
        text: String,

        /// Reply target.
        reply_to: Option<MessageId>,
    },
    /// Send one text Message, optionally as a reply.
    SendMessage {
        /// Destination Chat.
        chat: ChatId,
        /// Draft contents.
        text: String,
        /// Replied-to Message.
        reply_to: Option<MessageId>,

        /// Active Thread root, when sending inside a Thread.
        thread_root: Option<MessageId>,

        /// Adapter-owned attachments to upload.
        attachments: Vec<AttachmentId>,

        /// Optimistic local Message to acknowledge or fail.
        local_id: MessageId,
    },
    /// Shut down adapters and exit.
    Quit,
}

/// Immutable data rendered by a user interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct View {
    /// Current Telegram connectivity.
    pub connection: ConnectionState,

    /// Active Account display name.
    pub account_name: String,

    /// Synchronized Telegram Folders.
    pub folders: Vec<FolderView>,

    /// Active Folder index.
    pub active_folder: usize,

    /// Chats in the active Folder.
    pub chats: Vec<ChatView>,

    /// Active Chat index.
    pub active_chat: Option<usize>,

    /// Loaded messages for the active Chat.
    pub messages: Vec<MessageView>,

    /// Active Message index.
    pub active_message: Option<usize>,

    /// Active ordinary Thread or Channel comments root.
    pub active_thread: Option<MessageId>,

    /// Message index anchoring the Transcript when no Message is active.
    pub transcript_anchor: Option<usize>,

    /// Region receiving navigation and editing input.
    pub focus: Focus,

    /// Current Draft.
    pub composer: ComposerView,

    /// Active search, when open.
    pub search: Option<SearchView>,

    /// Whether unseen messages arrived while reading older history.
    pub has_newer_messages: bool,

    /// Whether exhaustive context help is open.
    pub help_open: bool,

    /// Latest nonfatal adapter notice.
    pub notice: Option<String>,

    /// Actions valid in the current context.
    pub actions: Vec<Action>,
}

/// One state transition observed by the composition root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Update {
    /// Immutable view after applying the input.
    pub view: View,
    /// Optional external work requested by the transition.
    pub effect: Option<Effect>,
}

/// Sole owner of mutable application state.
pub struct App {
    view: View,
    all_chats: Vec<ChatView>,
    drafts: HashMap<HistoryKey, ComposerView>,
    histories: HashMap<HistoryKey, Vec<MessageView>>,
    transcript_anchors: HashMap<HistoryKey, MessageId>,
    loading_history: Option<HistoryKey>,
    queued_history: Option<HistoryKey>,
    next_local_message_id: i64,
    pending_drafts: HashMap<MessageId, ComposerView>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct HistoryKey {
    chat: ChatId,
    thread: Option<MessageId>,
}

impl App {
    /// Creates an application waiting for initial adapter data.
    #[must_use]
    pub fn new() -> Self {
        let mut app = Self {
            view: View {
                connection: ConnectionState::Connecting,
                account_name: "Intuigram".to_owned(),
                folders: Vec::new(),
                active_folder: 0,
                chats: Vec::new(),
                active_chat: None,
                messages: Vec::new(),
                active_message: None,
                active_thread: None,
                transcript_anchor: None,
                focus: Focus::Chats,
                composer: ComposerView::default(),
                search: None,
                has_newer_messages: false,
                help_open: false,
                notice: None,
                actions: Vec::new(),
            },
            all_chats: Vec::new(),
            drafts: HashMap::new(),
            histories: HashMap::new(),
            transcript_anchors: HashMap::new(),
            loading_history: None,
            queued_history: None,
            next_local_message_id: 0,
            pending_drafts: HashMap::new(),
        };
        app.refresh_actions();
        app
    }

    /// Applies one ordered input and returns the resulting immutable view and
    /// adapter effect.
    #[must_use]
    pub fn transition(&mut self, input: Input) -> Update {
        let effect = self.apply(input);
        self.refresh_actions();
        Update {
            view: self.view.clone(),
            effect,
        }
    }

    /// Returns the current immutable view without changing application state.
    #[must_use]
    pub fn view(&self) -> View {
        self.view.clone()
    }

    fn apply(&mut self, input: Input) -> Option<Effect> {
        match input {
            Input::Adapter(AdapterEvent::Bootstrap(bootstrap)) => {
                self.view.connection = bootstrap.connection;
                self.view.account_name = bootstrap.account_name;
                self.view.folders = bootstrap.folders;
                self.all_chats = bootstrap.chats;
                self.drafts = bootstrap
                    .drafts
                    .into_iter()
                    .map(|draft| {
                        (
                            HistoryKey {
                                chat: draft.chat,
                                thread: draft.thread_root,
                            },
                            ComposerView {
                                text: draft.text,
                                reply_to: draft.reply_to,
                                attachments: Vec::new(),
                            },
                        )
                    })
                    .collect();
                self.refresh_folder_chats(None);
                self.view.active_chat = (!self.view.chats.is_empty()).then_some(0);
                self.histories = bootstrap
                    .histories
                    .into_iter()
                    .map(|history| {
                        (
                            HistoryKey {
                                chat: history.chat,
                                thread: history.thread_root,
                            },
                            history.messages,
                        )
                    })
                    .collect();
                self.transcript_anchors.clear();
                if let Some(chat) = self.active_chat_id() {
                    self.histories
                        .insert(HistoryKey { chat, thread: None }, bootstrap.messages);
                }
                self.view.active_message = None;
                self.view.active_thread = None;
                self.view.transcript_anchor = None;
                self.refresh_active_history();
                self.restore_active_draft();
                self.loading_history = None;
                self.queued_history = None;
                None
            }
            Input::Adapter(AdapterEvent::ConnectionChanged(connection)) => {
                self.view.connection = connection;
                None
            }
            Input::Adapter(AdapterEvent::ConnectionFailed(reason)) => {
                self.view.connection = ConnectionState::ReconnectCooldown;
                self.view.notice = Some(reason);
                None
            }
            Input::Adapter(AdapterEvent::MessageAdded { chat, message }) => {
                let active = self.active_chat_id() == Some(chat);
                let was_latest = active && self.at_latest();
                let active_message = active.then(|| self.active_message_id()).flatten();
                let transcript_anchor = active.then(|| self.transcript_anchor_id()).flatten();
                let visibly_read = active && self.view.focus != Focus::Chats && was_latest;
                let unread_increment =
                    u32::from(message.direction == MessageDirection::Incoming && !visibly_read);
                for chat_view in self
                    .all_chats
                    .iter_mut()
                    .chain(self.view.chats.iter_mut())
                    .filter(|view| view.id == chat)
                {
                    chat_view.preview.clone_from(&message.body);
                    chat_view.unread = chat_view.unread.saturating_add(unread_increment);
                }
                let reconciled = self.reconcile_pending_message(chat, &message);
                if !reconciled {
                    self.histories
                        .entry(HistoryKey { chat, thread: None })
                        .or_default()
                        .push((*message).clone());
                    if let Some(root) = message.details.thread_root {
                        self.histories
                            .entry(HistoryKey {
                                chat,
                                thread: Some(root),
                            })
                            .or_default()
                            .push(*message);
                    }
                }
                if active {
                    self.refresh_active_history_at(active_message, transcript_anchor);
                    self.view.has_newer_messages = !was_latest;
                }
                None
            }
            Input::Adapter(AdapterEvent::ChatLoaded { chat, messages }) => {
                let key = HistoryKey { chat, thread: None };
                if self.active_history_key() == Some(key) {
                    let active_message = self.active_message_id();
                    let transcript_anchor = self.transcript_anchor_id();
                    self.histories.insert(key, messages);
                    self.refresh_active_history_at(active_message, transcript_anchor);
                    self.view.has_newer_messages = false;
                } else {
                    self.histories.insert(key, messages);
                }
                self.complete_history_load(key)
            }
            Input::Adapter(AdapterEvent::ThreadLoaded {
                chat,
                root,
                messages,
            }) => {
                let key = HistoryKey {
                    chat,
                    thread: Some(root),
                };
                if self.active_history_key() == Some(key) {
                    let active_message = self.active_message_id();
                    let transcript_anchor = self.transcript_anchor_id();
                    self.histories.insert(key, messages);
                    self.refresh_active_history_at(active_message, transcript_anchor);
                    self.view.has_newer_messages = false;
                } else {
                    self.histories.insert(key, messages);
                }
                self.complete_history_load(key)
            }
            Input::Adapter(AdapterEvent::ClipboardReady {
                chat,
                thread_root,
                text,
                attachments,
            }) => {
                let key = HistoryKey {
                    chat,
                    thread: thread_root,
                };
                if self.active_history_key() == Some(key) {
                    if let Some(text) = text {
                        self.view.composer.text.push_str(&text);
                    }
                    self.view.composer.attachments.extend(attachments);
                    self.view.focus = Focus::Composer;
                    self.draft_effect()
                } else {
                    let draft = self.drafts.entry(key).or_default();
                    if let Some(text) = text {
                        draft.text.push_str(&text);
                    }
                    draft.attachments.extend(attachments);
                    Some(Effect::SaveDraft {
                        chat: key.chat,
                        thread_root: key.thread,
                        text: draft.text.clone(),
                        reply_to: draft.reply_to,
                    })
                }
            }
            Input::Adapter(AdapterEvent::MessageAcknowledged { chat, local_id }) => {
                self.update_delivery(chat, local_id, DeliveryState::Sent);
                self.pending_drafts.remove(&local_id);
                self.view.notice = None;
                None
            }
            Input::Adapter(AdapterEvent::MessageFailed {
                chat,
                local_id,
                thread_root,
                text,
                reason,
            }) => {
                self.update_delivery(chat, local_id, DeliveryState::Failed);
                let key = HistoryKey {
                    chat,
                    thread: thread_root,
                };
                let failed_draft = self
                    .pending_drafts
                    .remove(&local_id)
                    .unwrap_or(ComposerView {
                        text: text.clone(),
                        ..ComposerView::default()
                    });
                let (draft_text, draft_reply_to) = {
                    let draft = self.drafts.entry(key).or_default();
                    if draft.text.is_empty() && draft.attachments.is_empty() {
                        draft.clone_from(&failed_draft);
                    }
                    (draft.text.clone(), draft.reply_to)
                };
                if self.active_history_key() == Some(key)
                    && self.view.composer.text.is_empty()
                    && self.view.composer.attachments.is_empty()
                {
                    self.view.composer.clone_from(&failed_draft);
                }
                self.view.notice = Some(reason);
                Some(Effect::SaveDraft {
                    chat,
                    thread_root,
                    text: draft_text,
                    reply_to: draft_reply_to,
                })
            }
            Input::Intent(intent) => self.apply_intent(intent),
        }
    }

    fn apply_intent(&mut self, intent: Intent) -> Option<Effect> {
        match intent {
            Intent::Insert(text) => {
                if let Some(search) = &mut self.view.search {
                    search.query.push_str(&text);
                } else if self.view.active_chat.is_some() && self.view.focus != Focus::Chats {
                    self.focus_composer_at_anchor();
                    self.view.composer.text.push_str(&text);
                }
                self.draft_effect()
            }
            Intent::Backspace => {
                if let Some(search) = &mut self.view.search {
                    search.query.pop();
                } else if self.view.focus == Focus::Composer {
                    self.view.composer.text.pop();
                }
                self.draft_effect()
            }
            Intent::Action(action) => self.apply_action(action),
        }
    }

    fn apply_action(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::Quit => Some(Effect::Quit),
            Action::Help => {
                self.view.help_open = !self.view.help_open;
                None
            }
            Action::Reconnect if self.view.connection == ConnectionState::ReconnectCooldown => {
                self.view.connection = ConnectionState::Connecting;
                self.view.notice = None;
                Some(Effect::Reconnect)
            }
            Action::Reconnect => None,
            Action::MoveUp => self.move_chat(false),
            Action::MoveDown => self.move_chat(true),
            Action::PreviousFolder => {
                self.move_folder(false);
                None
            }
            Action::NextFolder => {
                self.move_folder(true);
                None
            }
            Action::Open => {
                if let Some(chat) = self.active_chat_id() {
                    self.focus_composer_at_anchor();
                    return self.request_chat_load(chat);
                }
                None
            }
            Action::Compose => {
                if self.view.active_chat.is_some() {
                    self.focus_composer_at_anchor();
                }
                None
            }
            Action::Reply => {
                self.view.composer.reply_to = self.active_message_id();
                if self.view.composer.reply_to.is_some() {
                    self.focus_composer_at_anchor();
                }
                self.draft_effect()
            }
            Action::OpenThread => self.open_thread(),
            Action::Paste => self.active_history_key().map(|key| Effect::ReadClipboard {
                chat: key.chat,
                thread_root: key.thread,
            }),
            Action::TargetPreviousMessage => {
                self.target_previous_message();
                None
            }
            Action::TargetNextMessage => {
                self.target_next_message();
                None
            }
            Action::Search => {
                let scope = if self.view.focus == Focus::Chats {
                    SearchScope::Account
                } else {
                    SearchScope::Chat
                };
                self.view.search = Some(SearchView {
                    scope,
                    query: String::new(),
                });
                self.view.focus = Focus::Search;
                None
            }
            Action::Cancel => {
                if self.view.help_open {
                    self.view.help_open = false;
                } else if let Some(search) = self.view.search.take() {
                    self.view.focus = match search.scope {
                        SearchScope::Account => Focus::Chats,
                        SearchScope::Chat => Focus::Composer,
                    };
                } else if self.view.composer.reply_to.take().is_some() {
                    self.view.focus = Focus::Composer;
                    return self.draft_effect();
                } else if self.view.focus == Focus::Transcript {
                    self.focus_composer_at_anchor();
                } else if self.view.focus == Focus::Composer {
                    if self.view.active_thread.is_some() {
                        self.leave_thread();
                    } else {
                        self.view.focus = Focus::Chats;
                    }
                }
                None
            }
            Action::JumpEarliest => {
                self.view.active_message = (!self.view.messages.is_empty()).then_some(0);
                self.view.transcript_anchor = self.view.active_message;
                self.view.focus = Focus::Transcript;
                None
            }
            Action::JumpLatest => {
                self.view.active_message = self.view.messages.len().checked_sub(1);
                self.view.transcript_anchor = self.view.active_message;
                self.view.has_newer_messages = false;
                self.view.focus = Focus::Transcript;
                None
            }
            Action::Send => self.send_message(),
            Action::Newline => {
                if self.view.focus == Focus::Composer {
                    self.view.composer.text.push('\n');
                }
                self.draft_effect()
            }
        }
    }

    fn send_message(&mut self) -> Option<Effect> {
        let chat_index = self.view.active_chat?;
        let chat = self.view.chats.get(chat_index)?.id;
        let text = self.view.composer.text.trim_end().to_owned();
        if text.is_empty() && self.view.composer.attachments.is_empty() {
            return None;
        }
        self.next_local_message_id = self.next_local_message_id.saturating_sub(1);
        let local_id = MessageId(self.next_local_message_id);
        self.pending_drafts
            .insert(local_id, self.view.composer.clone());
        let pending = MessageView {
            id: local_id,
            sender: "You".to_owned(),
            body: text.clone(),
            timestamp: "now".to_owned(),
            direction: MessageDirection::Outgoing,
            delivery: DeliveryState::Pending,
            reply_to: self.view.composer.reply_to,
            details: MessageDetails {
                thread_root: self.view.active_thread,
                ..MessageDetails::default()
            },
        };
        let key = self.active_history_key()?;
        self.histories.entry(key).or_default().push(pending);
        self.refresh_active_history();
        let effect = Effect::SendMessage {
            chat,
            text,
            reply_to: self.view.composer.reply_to,
            thread_root: self.view.active_thread,
            attachments: self
                .view
                .composer
                .attachments
                .iter()
                .map(|attachment| attachment.id)
                .collect(),
            local_id,
        };
        self.view.composer = ComposerView::default();
        if let Some(key) = self.active_history_key() {
            self.drafts.remove(&key);
        }
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.view.focus = Focus::Composer;
        Some(effect)
    }

    fn draft_effect(&self) -> Option<Effect> {
        let key = self.active_history_key()?;
        (self.view.focus == Focus::Composer).then(|| Effect::SaveDraft {
            chat: key.chat,
            thread_root: key.thread,
            text: self.view.composer.text.clone(),
            reply_to: self.view.composer.reply_to,
        })
    }

    fn update_delivery(&mut self, chat: ChatId, message: MessageId, delivery: DeliveryState) {
        for history in self
            .histories
            .iter_mut()
            .filter(|(key, _)| key.chat == chat)
            .map(|(_, history)| history)
        {
            if let Some(found) = history.iter_mut().find(|candidate| candidate.id == message) {
                found.delivery = delivery;
            }
        }
        if self.active_chat_id() == Some(chat) {
            self.refresh_active_history();
        }
    }

    fn reconcile_pending_message(&mut self, chat: ChatId, message: &MessageView) -> bool {
        if message.direction != MessageDirection::Outgoing || message.id.0 <= 0 {
            return false;
        }
        let mut reconciled = false;
        for history in self
            .histories
            .iter_mut()
            .filter(|(key, _)| key.chat == chat)
            .map(|(_, history)| history)
        {
            if let Some(pending) = history.iter_mut().rev().find(|candidate| {
                candidate.id.0 < 0
                    && candidate.direction == MessageDirection::Outgoing
                    && candidate.body == message.body
            }) {
                pending.clone_from(message);
                reconciled = true;
            }
        }
        reconciled
    }

    fn move_chat(&mut self, forward: bool) -> Option<Effect> {
        if self.view.focus != Focus::Chats {
            return None;
        }
        let next = move_index(self.view.active_chat, self.view.chats.len(), forward);
        if next == self.view.active_chat {
            return None;
        }
        self.save_active_draft();
        self.save_transcript_anchor();
        self.view.active_thread = None;
        self.view.active_chat = next;
        self.restore_active_draft();
        let transcript_anchor = self
            .active_history_key()
            .and_then(|key| self.transcript_anchors.get(&key).copied());
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.refresh_active_history_at(None, transcript_anchor);
        self.view.has_newer_messages = false;
        self.active_chat_id()
            .and_then(|chat| self.request_chat_load(chat))
    }

    fn request_chat_load(&mut self, chat: ChatId) -> Option<Effect> {
        self.request_history_load(HistoryKey { chat, thread: None })
    }

    fn request_history_load(&mut self, key: HistoryKey) -> Option<Effect> {
        match self.loading_history {
            None => {
                self.loading_history = Some(key);
                Some(match key.thread {
                    Some(root) => Effect::LoadThread {
                        chat: key.chat,
                        root,
                    },
                    None => Effect::LoadChat { chat: key.chat },
                })
            }
            Some(loading) if loading == key => {
                self.queued_history = None;
                None
            }
            Some(_) => {
                self.queued_history = Some(key);
                None
            }
        }
    }

    fn complete_history_load(&mut self, key: HistoryKey) -> Option<Effect> {
        if self.loading_history != Some(key) {
            return None;
        }
        self.loading_history = None;
        self.queued_history
            .take()
            .filter(|queued| *queued != key)
            .and_then(|queued| self.request_history_load(queued))
    }

    fn move_folder(&mut self, forward: bool) {
        if self.view.focus == Focus::Chats {
            let active_chat = self.active_chat_id();
            self.save_active_draft();
            self.save_transcript_anchor();
            self.view.active_folder = move_index(
                Some(self.view.active_folder),
                self.view.folders.len(),
                forward,
            )
            .unwrap_or(0);
            self.refresh_folder_chats(active_chat);
            self.restore_active_draft();
            self.view.active_thread = None;
            let transcript_anchor = self
                .active_history_key()
                .and_then(|key| self.transcript_anchors.get(&key).copied());
            self.view.active_message = None;
            self.view.transcript_anchor = None;
            self.refresh_active_history_at(None, transcript_anchor);
        }
    }

    fn refresh_folder_chats(&mut self, preferred: Option<ChatId>) {
        let folder = self
            .view
            .folders
            .get(self.view.active_folder)
            .map_or(0, |folder| folder.id);
        self.view.chats = self
            .all_chats
            .iter()
            .filter(|chat| chat.folders.contains(&folder))
            .cloned()
            .collect();
        self.view.active_chat = preferred
            .and_then(|chat| {
                self.view
                    .chats
                    .iter()
                    .position(|candidate| candidate.id == chat)
            })
            .or_else(|| (!self.view.chats.is_empty()).then_some(0));
    }

    fn target_previous_message(&mut self) {
        if self.view.messages.is_empty() {
            return;
        }
        self.view.active_message = Some(
            match self.view.active_message.or(self.view.transcript_anchor) {
                Some(index) => index.saturating_sub(1),
                None => self.view.messages.len() - 1,
            },
        );
        self.view.transcript_anchor = self.view.active_message;
        self.view.focus = Focus::Transcript;
    }

    fn open_thread(&mut self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        let root = self.active_message_id()?;
        self.save_active_draft();
        self.save_transcript_anchor();
        self.view.active_thread = Some(root);
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.restore_active_draft();
        self.refresh_active_history();
        self.view.focus = Focus::Composer;
        self.request_history_load(HistoryKey {
            chat,
            thread: Some(root),
        })
    }

    fn leave_thread(&mut self) {
        self.save_active_draft();
        self.save_transcript_anchor();
        self.view.active_thread = None;
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.restore_active_draft();
        let anchor = self
            .active_history_key()
            .and_then(|key| self.transcript_anchors.get(&key).copied());
        self.refresh_active_history_at(None, anchor);
        self.view.focus = Focus::Composer;
    }

    fn target_next_message(&mut self) {
        if self.view.focus != Focus::Transcript {
            return;
        }
        let Some(index) = self.view.active_message else {
            self.view.focus = Focus::Composer;
            return;
        };
        if index + 1 < self.view.messages.len() {
            self.view.active_message = Some(index + 1);
            self.view.transcript_anchor = self.view.active_message;
        } else {
            self.view.active_message = None;
            self.view.transcript_anchor = self.view.messages.len().checked_sub(1);
            self.view.has_newer_messages = false;
            self.view.focus = Focus::Composer;
        }
    }

    fn save_active_draft(&mut self) {
        if let Some(key) = self.active_history_key() {
            self.drafts.insert(key, self.view.composer.clone());
        }
    }

    fn focus_composer_at_anchor(&mut self) {
        if self.view.active_message.is_some() {
            self.view.transcript_anchor = self.view.active_message;
        }
        self.view.active_message = None;
        self.view.focus = Focus::Composer;
    }

    fn restore_active_draft(&mut self) {
        self.view.composer = self
            .active_history_key()
            .and_then(|key| self.drafts.get(&key).cloned())
            .unwrap_or_default();
    }

    fn save_transcript_anchor(&mut self) {
        let Some(key) = self.active_history_key() else {
            return;
        };
        if let Some(anchor) = self.transcript_anchor_id() {
            self.transcript_anchors.insert(key, anchor);
        } else {
            self.transcript_anchors.remove(&key);
        }
    }

    fn refresh_active_history(&mut self) {
        let active_message = self.active_message_id();
        let transcript_anchor = self.transcript_anchor_id();
        self.refresh_active_history_at(active_message, transcript_anchor);
    }

    fn refresh_active_history_at(
        &mut self,
        active_message: Option<MessageId>,
        transcript_anchor: Option<MessageId>,
    ) {
        self.view.messages = self
            .active_history_key()
            .and_then(|key| self.histories.get(&key).cloned())
            .unwrap_or_default();
        self.view.active_message =
            active_message.and_then(|message| self.history_position(message));
        self.view.transcript_anchor =
            transcript_anchor.and_then(|message| self.history_position(message));
    }

    fn history_position(&self, message: MessageId) -> Option<usize> {
        self.view
            .messages
            .iter()
            .position(|candidate| candidate.id == message)
    }

    fn active_message_id(&self) -> Option<MessageId> {
        self.view
            .active_message
            .and_then(|index| self.view.messages.get(index))
            .map(|message| message.id)
    }

    fn transcript_anchor_id(&self) -> Option<MessageId> {
        self.view
            .active_message
            .or(self.view.transcript_anchor)
            .and_then(|index| self.view.messages.get(index))
            .map(|message| message.id)
    }

    fn active_chat_id(&self) -> Option<ChatId> {
        self.view
            .active_chat
            .and_then(|index| self.view.chats.get(index))
            .map(|chat| chat.id)
    }

    fn active_history_key(&self) -> Option<HistoryKey> {
        self.active_chat_id().map(|chat| HistoryKey {
            chat,
            thread: self.view.active_thread,
        })
    }

    fn at_latest(&self) -> bool {
        self.view
            .active_message
            .or(self.view.transcript_anchor)
            .is_none_or(|index| Some(index) == self.view.messages.len().checked_sub(1))
    }

    fn refresh_actions(&mut self) {
        let mut actions = vec![Action::Quit, Action::Help];
        if self.view.help_open {
            self.view.actions = vec![Action::Quit, Action::Help, Action::Cancel];
            return;
        }
        match self.view.focus {
            Focus::Chats => {
                actions.extend([
                    Action::MoveUp,
                    Action::MoveDown,
                    Action::PreviousFolder,
                    Action::NextFolder,
                    Action::Open,
                    Action::Search,
                ]);
            }
            Focus::Transcript => {
                actions.extend([
                    Action::TargetPreviousMessage,
                    Action::TargetNextMessage,
                    Action::Compose,
                    Action::Reply,
                    Action::OpenThread,
                    Action::Search,
                    Action::JumpEarliest,
                    Action::JumpLatest,
                ]);
            }
            Focus::Composer => {
                actions.extend([
                    Action::Send,
                    Action::Newline,
                    Action::Paste,
                    Action::Cancel,
                    Action::Search,
                    Action::TargetPreviousMessage,
                ]);
            }
            Focus::Search => actions.push(Action::Cancel),
        }
        if self.view.connection == ConnectionState::ReconnectCooldown {
            actions.push(Action::Reconnect);
        }
        self.view.actions = actions;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn move_index(current: Option<usize>, length: usize, forward: bool) -> Option<usize> {
    if length == 0 {
        return None;
    }
    let current = current.unwrap_or(0).min(length - 1);
    Some(if forward {
        (current + 1).min(length - 1)
    } else {
        current.saturating_sub(1)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Action, AdapterEvent, App, Bootstrap, ChatId, ChatKind, ChatView, ConnectionState,
        DeliveryState, Effect, Focus, FolderView, Input, Intent, MessageDirection, MessageId,
        MessageView, SearchScope,
    };

    fn bootstrap() -> Bootstrap {
        Bootstrap {
            connection: ConnectionState::Connected,
            account_name: "Ada".to_owned(),
            folders: vec![FolderView {
                id: 0,
                title: "All".to_owned(),
                unread: 2,
            }],
            chats: vec![ChatView {
                id: ChatId(10),
                title: "Intuigram".to_owned(),
                preview: "daily driver".to_owned(),
                unread: 2,
                pinned: true,
                kind: ChatKind::Supergroup,
                folders: vec![0],
            }],
            messages: (1..=3)
                .map(|id| MessageView {
                    id: MessageId(id),
                    sender: "Lin".to_owned(),
                    body: format!("message {id}"),
                    timestamp: "12:00".to_owned(),
                    direction: MessageDirection::Incoming,
                    delivery: DeliveryState::Read,
                    reply_to: None,
                    details: super::MessageDetails::default(),
                })
                .collect(),
            drafts: Vec::new(),
            histories: Vec::new(),
        }
    }

    fn hierarchy_bootstrap() -> Bootstrap {
        let mut fixture = bootstrap();
        fixture.folders.push(FolderView {
            id: 1,
            title: "Work".to_owned(),
            unread: 0,
        });
        fixture.chats.push(ChatView {
            id: ChatId(20),
            title: "Rust".to_owned(),
            preview: "owned buffers".to_owned(),
            unread: 0,
            pinned: false,
            kind: ChatKind::Supergroup,
            folders: vec![0, 1],
        });
        fixture
    }

    fn apply(app: &mut App, input: Input) {
        drop(app.transition(input));
    }

    #[test]
    fn reducer_applies_one_input_synchronously() {
        let mut app = App::new();

        let update = app.transition(Input::Adapter(AdapterEvent::Bootstrap(bootstrap())));

        assert_eq!(update.view.account_name, "Ada");
        assert_eq!(update.view.connection, ConnectionState::Connected);
        assert_eq!(update.effect, None);
    }

    #[test]
    fn bootstrap_restores_persisted_root_and_thread_drafts() {
        let mut fixture = bootstrap();
        fixture.drafts = vec![
            super::DraftView {
                chat: ChatId(10),
                thread_root: None,
                text: "root draft".to_owned(),
                reply_to: Some(MessageId(2)),
            },
            super::DraftView {
                chat: ChatId(10),
                thread_root: Some(MessageId(3)),
                text: "thread draft".to_owned(),
                reply_to: None,
            },
        ];
        let mut app = App::new();

        apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));
        apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
        assert_eq!(app.view().composer.text, "root draft");
        assert_eq!(app.view().composer.reply_to, Some(MessageId(2)));

        apply(&mut app, Input::Intent(Intent::Action(Action::JumpLatest)));
        apply(&mut app, Input::Intent(Intent::Action(Action::OpenThread)));
        assert_eq!(app.view().composer.text, "thread draft");
    }

    #[test]
    fn switching_folder_rebuilds_the_chat_list_from_normalized_membership() {
        let mut fixture = hierarchy_bootstrap();
        fixture.chats[0].folders = vec![0];
        fixture.chats[1].folders = vec![0, 1];
        let mut app = App::new();
        apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

        apply(&mut app, Input::Intent(Intent::Action(Action::NextFolder)));

        let view = app.view();
        assert_eq!(view.active_folder, 1);
        assert_eq!(view.chats.len(), 1);
        assert_eq!(view.chats[0].id, ChatId(20));
        assert_eq!(view.active_chat, Some(0));
    }

    #[test]
    fn new_messages_do_not_snap_transcript_while_reading_older_history() {
        let mut app = App::new();
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
        );
        apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
        apply(&mut app, Input::Intent(Intent::Action(Action::JumpLatest)));
        let older = app.transition(Input::Intent(Intent::Action(Action::TargetPreviousMessage)));
        assert_eq!(older.view.active_message, Some(1));

        let updated = app.transition(Input::Adapter(AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(MessageView {
                id: MessageId(4),
                sender: "Lin".to_owned(),
                body: "new".to_owned(),
                timestamp: "12:01".to_owned(),
                direction: MessageDirection::Incoming,
                delivery: DeliveryState::Sent,
                reply_to: None,
                details: super::MessageDetails::default(),
            }),
        }));

        assert_eq!(updated.view.active_message, Some(1));
        assert!(updated.view.has_newer_messages);
    }

    #[test]
    fn passive_message_updates_refresh_the_chat_list_without_a_history_reload() {
        let mut app = App::new();
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
        );

        let updated = app.transition(Input::Adapter(AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(MessageView {
                id: MessageId(4),
                sender: "Lin".to_owned(),
                body: "live update".to_owned(),
                timestamp: "12:02".to_owned(),
                direction: MessageDirection::Incoming,
                delivery: DeliveryState::Sent,
                reply_to: None,
                details: super::MessageDetails::default(),
            }),
        }));

        assert_eq!(updated.view.chats[0].preview, "live update");
        assert_eq!(updated.view.chats[0].unread, 3);
        assert_eq!(
            updated.view.messages.last().map(|message| message.id),
            Some(MessageId(4))
        );
    }

    #[test]
    fn search_scope_and_reply_send_follow_current_context() {
        let mut app = App::new();
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
        );
        let search = app.transition(Input::Intent(Intent::Action(Action::Search)));
        assert_eq!(
            search.view.search.expect("search should be open").scope,
            SearchScope::Account
        );
        for action in [
            Action::Cancel,
            Action::Open,
            Action::TargetPreviousMessage,
            Action::Reply,
        ] {
            apply(&mut app, Input::Intent(Intent::Action(action)));
        }
        apply(&mut app, Input::Intent(Intent::Insert("hello".to_owned())));
        apply(&mut app, Input::Intent(Intent::Action(Action::Newline)));
        apply(&mut app, Input::Intent(Intent::Insert("world".to_owned())));
        let sent = app.transition(Input::Intent(Intent::Action(Action::Send)));
        assert_eq!(
            sent.effect,
            Some(Effect::SendMessage {
                chat: ChatId(10),
                text: "hello\nworld".to_owned(),
                reply_to: Some(MessageId(3)),
                thread_root: None,
                attachments: Vec::new(),
                local_id: MessageId(-1),
            })
        );
        assert_eq!(
            sent.view.messages.last().map(|message| message.delivery),
            Some(DeliveryState::Pending)
        );
        assert_eq!(sent.view.focus, Focus::Composer);
    }

    #[test]
    fn failed_optimistic_send_restores_the_draft_and_marks_the_message_failed() {
        let mut app = App::new();
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
        );
        apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
        apply(
            &mut app,
            Input::Intent(Intent::Insert("retry me".to_owned())),
        );

        let sent = app.transition(Input::Intent(Intent::Action(Action::Send)));
        let local_id = match sent.effect {
            Some(Effect::SendMessage { local_id, .. }) => local_id,
            effect => panic!("expected optimistic send effect, got {effect:?}"),
        };
        assert!(sent.view.composer.text.is_empty());
        assert_eq!(
            sent.view.messages.last().map(|message| message.delivery),
            Some(DeliveryState::Pending)
        );

        let failed = app.transition(Input::Adapter(AdapterEvent::MessageFailed {
            chat: ChatId(10),
            local_id,
            thread_root: None,
            text: "retry me".to_owned(),
            reason: "Telegram is unavailable".to_owned(),
        }));

        assert_eq!(failed.view.composer.text, "retry me");
        assert_eq!(
            failed.view.notice.as_deref(),
            Some("Telegram is unavailable")
        );
        assert_eq!(
            failed.view.messages.last().map(|message| message.delivery),
            Some(DeliveryState::Failed)
        );
        assert_eq!(
            failed.effect,
            Some(Effect::SaveDraft {
                chat: ChatId(10),
                thread_root: None,
                text: "retry me".to_owned(),
                reply_to: None,
            })
        );
    }

    #[test]
    fn thread_navigation_preserves_parent_history_and_an_independent_draft() {
        let mut app = App::new();
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
        );
        apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::ChatLoaded {
                chat: ChatId(10),
                messages: bootstrap().messages,
            }),
        );
        apply(
            &mut app,
            Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
        );
        let opened = app.transition(Input::Intent(Intent::Action(Action::OpenThread)));
        assert_eq!(
            opened.effect,
            Some(Effect::LoadThread {
                chat: ChatId(10),
                root: MessageId(3),
            })
        );
        assert_eq!(opened.view.active_thread, Some(MessageId(3)));
        assert!(opened.view.messages.is_empty());
        let thread_message = MessageView {
            id: MessageId(4),
            sender: "Lin".to_owned(),
            body: "thread reply".to_owned(),
            timestamp: "12:03".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: Some(MessageId(3)),
            details: super::MessageDetails {
                thread_root: Some(MessageId(3)),
                ..super::MessageDetails::default()
            },
        };
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::ThreadLoaded {
                chat: ChatId(10),
                root: MessageId(3),
                messages: vec![thread_message],
            }),
        );
        apply(
            &mut app,
            Input::Intent(Intent::Insert("thread draft".to_owned())),
        );
        apply(&mut app, Input::Intent(Intent::Action(Action::Cancel)));
        let parent = app.view();
        assert_eq!(parent.active_thread, None);
        assert_eq!(parent.messages.len(), 3);
        assert!(parent.composer.text.is_empty());

        apply(&mut app, Input::Intent(Intent::Action(Action::JumpLatest)));
        apply(&mut app, Input::Intent(Intent::Action(Action::OpenThread)));
        assert_eq!(app.view().composer.text, "thread draft");
        assert_eq!(app.view().messages.len(), 1);
    }

    #[test]
    fn chat_movement_changes_active_chat_and_preserves_each_draft() {
        let mut app = App::new();
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::Bootstrap(hierarchy_bootstrap())),
        );
        let opened = app.transition(Input::Intent(Intent::Action(Action::Open)));
        assert_eq!(opened.view.focus, Focus::Composer);
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::ChatLoaded {
                chat: ChatId(10),
                messages: hierarchy_bootstrap().messages,
            }),
        );
        apply(
            &mut app,
            Input::Intent(Intent::Insert("first draft".to_owned())),
        );
        apply(&mut app, Input::Intent(Intent::Action(Action::Cancel)));
        let second = app.transition(Input::Intent(Intent::Action(Action::MoveDown)));
        assert_eq!(second.view.active_chat, Some(1));
        assert!(second.view.messages.is_empty());
        assert!(second.view.composer.text.is_empty());
        assert_eq!(second.effect, Some(Effect::LoadChat { chat: ChatId(20) }));
        let first = app.transition(Input::Intent(Intent::Action(Action::MoveUp)));
        assert_eq!(first.view.active_chat, Some(0));
        assert_eq!(first.view.messages, hierarchy_bootstrap().messages);
        assert_eq!(first.view.composer.text, "first draft");
        assert_eq!(first.effect, None);

        let queued = app.transition(Input::Adapter(AdapterEvent::ChatLoaded {
            chat: ChatId(20),
            messages: Vec::new(),
        }));
        assert_eq!(queued.effect, Some(Effect::LoadChat { chat: ChatId(10) }));
    }

    #[test]
    fn revisiting_a_loaded_chat_renders_cached_history_while_refreshing() {
        let mut app = App::new();
        let initial = hierarchy_bootstrap().messages;
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::Bootstrap(hierarchy_bootstrap())),
        );

        let second = app.transition(Input::Intent(Intent::Action(Action::MoveDown)));
        assert!(second.view.messages.is_empty());
        assert_eq!(second.effect, Some(Effect::LoadChat { chat: ChatId(20) }));

        let second_history = vec![MessageView {
            id: MessageId(20),
            sender: "Ferris".to_owned(),
            body: "cached second chat".to_owned(),
            timestamp: "12:20".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: None,
            details: super::MessageDetails::default(),
        }];
        let loaded = app.transition(Input::Adapter(AdapterEvent::ChatLoaded {
            chat: ChatId(20),
            messages: second_history.clone(),
        }));
        assert_eq!(loaded.view.messages, second_history);

        let first = app.transition(Input::Intent(Intent::Action(Action::MoveUp)));
        assert_eq!(first.view.messages, initial);
        assert_eq!(first.effect, Some(Effect::LoadChat { chat: ChatId(10) }));

        let mut refreshed = hierarchy_bootstrap().messages;
        refreshed.push(MessageView {
            id: MessageId(4),
            sender: "Lin".to_owned(),
            body: "arrived while away".to_owned(),
            timestamp: "12:21".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Sent,
            reply_to: None,
            details: super::MessageDetails::default(),
        });
        let refreshed_view = app.transition(Input::Adapter(AdapterEvent::ChatLoaded {
            chat: ChatId(10),
            messages: refreshed.clone(),
        }));
        assert_eq!(refreshed_view.view.messages, refreshed);
    }

    #[test]
    fn bootstrap_cached_history_renders_before_a_background_refresh() {
        let mut fixture = hierarchy_bootstrap();
        let cached = MessageView {
            id: MessageId(20),
            sender: "Ferris".to_owned(),
            body: "durable cached history".to_owned(),
            timestamp: "12:20".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: None,
            details: super::MessageDetails::default(),
        };
        fixture.histories.push(super::HistoryView {
            chat: ChatId(20),
            thread_root: None,
            messages: vec![cached.clone()],
        });
        let mut app = App::new();
        apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

        let switched = app.transition(Input::Intent(Intent::Action(Action::MoveDown)));

        assert_eq!(switched.view.messages, vec![cached]);
        assert_eq!(switched.effect, Some(Effect::LoadChat { chat: ChatId(20) }));
    }

    #[test]
    fn delayed_message_results_update_their_destination_chat_only() {
        let mut app = App::new();
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::Bootstrap(hierarchy_bootstrap())),
        );
        apply(&mut app, Input::Intent(Intent::Action(Action::MoveDown)));

        let delayed = app.transition(Input::Adapter(AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(MessageView {
                id: MessageId(4),
                sender: "You".to_owned(),
                body: "sent before switching".to_owned(),
                timestamp: "12:22".to_owned(),
                direction: MessageDirection::Outgoing,
                delivery: DeliveryState::Sent,
                reply_to: None,
                details: super::MessageDetails::default(),
            }),
        }));
        assert!(delayed.view.messages.is_empty());

        let first = app.transition(Input::Intent(Intent::Action(Action::MoveUp)));
        assert_eq!(
            first.view.messages.last().map(|message| message.id),
            Some(MessageId(4))
        );
    }

    #[test]
    fn returning_to_the_composer_preserves_the_older_transcript_anchor() {
        let mut app = App::new();
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
        );
        apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
        apply(
            &mut app,
            Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
        );
        apply(
            &mut app,
            Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
        );

        let composer = app.transition(Input::Intent(Intent::Action(Action::Cancel)));
        assert_eq!(composer.view.focus, Focus::Composer);
        assert_eq!(composer.view.active_message, None);
        assert_eq!(composer.view.transcript_anchor, Some(1));
    }

    #[test]
    fn escape_ascends_the_hierarchy_and_folders_change_only_from_chat_list() {
        let mut app = App::new();
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::Bootstrap(hierarchy_bootstrap())),
        );
        let chat_list = app.transition(Input::Intent(Intent::Insert("does not enter".to_owned())));
        assert_eq!(chat_list.view.focus, Focus::Chats);
        assert!(chat_list.view.composer.text.is_empty());
        apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
        let composer = app.transition(Input::Intent(Intent::Action(Action::NextFolder)));
        assert_eq!(composer.view.active_folder, 0);
        let targeted = app.transition(Input::Intent(Intent::Action(Action::TargetPreviousMessage)));
        assert_eq!(targeted.view.focus, Focus::Transcript);
        let newest = app.transition(Input::Intent(Intent::Action(Action::TargetNextMessage)));
        assert_eq!(newest.view.focus, Focus::Composer);
        apply(
            &mut app,
            Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
        );
        let composer = app.transition(Input::Intent(Intent::Action(Action::Cancel)));
        assert_eq!(composer.view.focus, Focus::Composer);
        let chats = app.transition(Input::Intent(Intent::Action(Action::Cancel)));
        assert_eq!(chats.view.focus, Focus::Chats);
        let folder = app.transition(Input::Intent(Intent::Action(Action::NextFolder)));
        assert_eq!(folder.view.active_folder, 1);
    }

    #[test]
    fn reconnect_is_available_only_during_cooldown() {
        let mut app = App::new();
        assert!(!app.view().actions.contains(&Action::Reconnect));
        let cooldown = app.transition(Input::Adapter(AdapterEvent::ConnectionChanged(
            ConnectionState::ReconnectCooldown,
        )));
        assert!(cooldown.view.actions.contains(&Action::Reconnect));
        let reconnecting = app.transition(Input::Intent(Intent::Action(Action::Reconnect)));
        assert_eq!(reconnecting.view.connection, ConnectionState::Connecting);
        assert!(!reconnecting.view.actions.contains(&Action::Reconnect));
        assert_eq!(reconnecting.effect, Some(Effect::Reconnect));
    }
}
