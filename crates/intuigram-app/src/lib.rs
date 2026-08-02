//! Deterministic, single-owner application state for Intuigram.

use std::collections::HashMap;

/// Stable identifier for a Telegram chat.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChatId(pub i64);

/// Stable identifier for a Telegram message.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MessageId(pub i64);

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
}

/// Current Draft state for the active Chat.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ComposerView {
    /// Draft text.
    pub text: String,
    /// Message targeted by a reply.
    pub reply_to: Option<MessageId>,
}

/// Active search query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchView {
    /// Search scope selected from the prior focus.
    pub scope: SearchScope,
    /// Query entered so far.
    pub query: String,
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
    /// Reply to the Active Message.
    Reply,
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
    /// Active Account display name.
    pub account_name: String,
    /// Synchronized Telegram Folders.
    pub folders: Vec<FolderView>,
    /// Chats in the active Folder.
    pub chats: Vec<ChatView>,
    /// Messages for the initially active Chat.
    pub messages: Vec<MessageView>,
}

/// Results reported by external adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterEvent {
    /// Initial synchronized data became available.
    Bootstrap(Bootstrap),
    /// Telegram connectivity changed.
    ConnectionChanged(ConnectionState),
    /// A new or acknowledged Message belongs in a Chat history.
    MessageAdded {
        /// Chat that owns the Message.
        chat: ChatId,
        /// Newly available Message.
        message: MessageView,
    },
    /// A requested Chat history became available.
    ChatLoaded {
        /// Chat whose history was loaded.
        chat: ChatId,
        /// Chronological loaded history.
        messages: Vec<MessageView>,
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
    /// Send one text Message, optionally as a reply.
    SendMessage {
        /// Destination Chat.
        chat: ChatId,
        /// Draft contents.
        text: String,
        /// Replied-to Message.
        reply_to: Option<MessageId>,
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
    drafts: HashMap<ChatId, ComposerView>,
    histories: HashMap<ChatId, Vec<MessageView>>,
    transcript_anchors: HashMap<ChatId, MessageId>,
    loading_chat: Option<ChatId>,
    queued_chat: Option<ChatId>,
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
                transcript_anchor: None,
                focus: Focus::Chats,
                composer: ComposerView::default(),
                search: None,
                has_newer_messages: false,
                help_open: false,
                actions: Vec::new(),
            },
            drafts: HashMap::new(),
            histories: HashMap::new(),
            transcript_anchors: HashMap::new(),
            loading_chat: None,
            queued_chat: None,
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
                self.view.connection = ConnectionState::Connected;
                self.view.account_name = bootstrap.account_name;
                self.view.folders = bootstrap.folders;
                self.view.chats = bootstrap.chats;
                self.view.active_chat = (!self.view.chats.is_empty()).then_some(0);
                self.histories.clear();
                self.transcript_anchors.clear();
                if let Some(chat) = self.active_chat_id() {
                    self.histories.insert(chat, bootstrap.messages);
                }
                self.view.active_message = None;
                self.view.transcript_anchor = None;
                self.refresh_active_history();
                self.loading_chat = None;
                self.queued_chat = None;
                None
            }
            Input::Adapter(AdapterEvent::ConnectionChanged(connection)) => {
                self.view.connection = connection;
                None
            }
            Input::Adapter(AdapterEvent::MessageAdded { chat, message }) => {
                let active = self.active_chat_id() == Some(chat);
                let was_latest = active && self.at_latest();
                let active_message = active.then(|| self.active_message_id()).flatten();
                let transcript_anchor = active.then(|| self.transcript_anchor_id()).flatten();
                self.histories.entry(chat).or_default().push(message);
                if active {
                    self.refresh_active_history_at(active_message, transcript_anchor);
                    self.view.has_newer_messages = !was_latest;
                }
                None
            }
            Input::Adapter(AdapterEvent::ChatLoaded { chat, messages }) => {
                if self.active_chat_id() == Some(chat) {
                    let active_message = self.active_message_id();
                    let transcript_anchor = self.transcript_anchor_id();
                    self.histories.insert(chat, messages);
                    self.refresh_active_history_at(active_message, transcript_anchor);
                    self.view.has_newer_messages = false;
                } else {
                    self.histories.insert(chat, messages);
                }

                if self.loading_chat != Some(chat) {
                    return None;
                }

                self.loading_chat = None;
                self.queued_chat
                    .take()
                    .filter(|queued| *queued != chat)
                    .and_then(|queued| self.request_chat_load(queued))
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
                None
            }
            Intent::Backspace => {
                if let Some(search) = &mut self.view.search {
                    search.query.pop();
                } else if self.view.focus == Focus::Composer {
                    self.view.composer.text.pop();
                }
                None
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
                None
            }
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
                } else if self.view.focus == Focus::Transcript {
                    self.focus_composer_at_anchor();
                } else if self.view.focus == Focus::Composer {
                    self.view.focus = Focus::Chats;
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
                None
            }
        }
    }

    fn send_message(&mut self) -> Option<Effect> {
        let chat_index = self.view.active_chat?;
        let chat = self.view.chats.get(chat_index)?.id;
        let text = self.view.composer.text.trim_end().to_owned();
        if text.is_empty() {
            return None;
        }
        let effect = Effect::SendMessage {
            chat,
            text,
            reply_to: self.view.composer.reply_to,
        };
        self.view.composer = ComposerView::default();
        self.drafts.remove(&chat);
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.view.focus = Focus::Composer;
        Some(effect)
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
        self.view.active_chat = next;
        self.restore_active_draft();
        let transcript_anchor = self
            .active_chat_id()
            .and_then(|chat| self.transcript_anchors.get(&chat).copied());
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.refresh_active_history_at(None, transcript_anchor);
        self.view.has_newer_messages = false;
        self.active_chat_id()
            .and_then(|chat| self.request_chat_load(chat))
    }

    fn request_chat_load(&mut self, chat: ChatId) -> Option<Effect> {
        match self.loading_chat {
            None => {
                self.loading_chat = Some(chat);
                Some(Effect::LoadChat { chat })
            }
            Some(loading) if loading == chat => {
                self.queued_chat = None;
                None
            }
            Some(_) => {
                self.queued_chat = Some(chat);
                None
            }
        }
    }

    fn move_folder(&mut self, forward: bool) {
        if self.view.focus == Focus::Chats {
            self.view.active_folder = move_index(
                Some(self.view.active_folder),
                self.view.folders.len(),
                forward,
            )
            .unwrap_or(0);
        }
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
        if let Some(chat) = self.active_chat_id() {
            self.drafts.insert(chat, self.view.composer.clone());
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
            .active_chat_id()
            .and_then(|chat| self.drafts.get(&chat).cloned())
            .unwrap_or_default();
    }

    fn save_transcript_anchor(&mut self) {
        let Some(chat) = self.active_chat_id() else {
            return;
        };
        if let Some(anchor) = self.transcript_anchor_id() {
            self.transcript_anchors.insert(chat, anchor);
        } else {
            self.transcript_anchors.remove(&chat);
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
            .active_chat_id()
            .and_then(|chat| self.histories.get(&chat).cloned())
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
                    Action::Search,
                    Action::JumpEarliest,
                    Action::JumpLatest,
                ]);
            }
            Focus::Composer => {
                actions.extend([
                    Action::Send,
                    Action::Newline,
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
        Action, AdapterEvent, App, Bootstrap, ChatId, ChatView, ConnectionState, DeliveryState,
        Effect, Focus, FolderView, Input, Intent, MessageDirection, MessageId, MessageView,
        SearchScope,
    };

    fn bootstrap() -> Bootstrap {
        Bootstrap {
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
                })
                .collect(),
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
    fn new_messages_do_not_snap_transcript_while_reading_older_history() {
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
        let older = app.transition(Input::Intent(Intent::Action(Action::TargetPreviousMessage)));
        assert_eq!(older.view.active_message, Some(1));

        let updated = app.transition(Input::Adapter(AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: MessageView {
                id: MessageId(4),
                sender: "Lin".to_owned(),
                body: "new".to_owned(),
                timestamp: "12:01".to_owned(),
                direction: MessageDirection::Incoming,
                delivery: DeliveryState::Sent,
                reply_to: None,
            },
        }));

        assert_eq!(updated.view.active_message, Some(1));
        assert!(updated.view.has_newer_messages);
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
            })
        );
        assert_eq!(sent.view.focus, Focus::Composer);
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
        });
        let refreshed_view = app.transition(Input::Adapter(AdapterEvent::ChatLoaded {
            chat: ChatId(10),
            messages: refreshed.clone(),
        }));
        assert_eq!(refreshed_view.view.messages, refreshed);
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
            message: MessageView {
                id: MessageId(4),
                sender: "You".to_owned(),
                body: "sent before switching".to_owned(),
                timestamp: "12:22".to_owned(),
                direction: MessageDirection::Outgoing,
                delivery: DeliveryState::Sent,
                reply_to: None,
            },
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
