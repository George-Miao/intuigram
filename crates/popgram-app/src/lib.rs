//! Deterministic, single-owner application state for Popgram.

use std::num::NonZeroUsize;

use async_channel::{Receiver, Sender};
use snafu::Snafu;

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

/// Interface region receiving unmodified navigation and editing keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Focus {
    /// Telegram Folder strip.
    Folders,
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
    /// Exit Popgram cleanly.
    Quit,
    /// Open exhaustive context help.
    Help,
    /// Advance focus without entering a keyboard mode.
    FocusNext,
    /// Move the active item upward.
    MoveUp,
    /// Move the active item downward.
    MoveDown,
    /// Open the active Chat or focus the Transcript.
    Open,
    /// Focus the Draft editor.
    Compose,
    /// Send the current Draft.
    Send,
    /// Reply to the Active Message.
    Reply,
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
    /// Insert a newline into the Draft.
    Newline,
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
    /// A new or acknowledged Message belongs in the active Transcript.
    MessageAdded(MessageView),
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

/// Failure while running the state owner.
#[derive(Debug, Snafu)]
pub enum Error {
    /// All view consumers disconnected while the application was running.
    #[snafu(display("application output channel closed"))]
    OutputClosed,
}

/// Result returned by the application state owner.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// UI and adapter endpoints for a bounded application channel pair.
pub struct AppHandle {
    /// Ordered input producer.
    pub inputs: Sender<Input>,
    /// Immutable updates from the state owner.
    pub updates: Receiver<Update>,
}

/// State-owner endpoints that can only be created as a bounded pair.
pub struct AppChannels {
    inputs: Receiver<Input>,
    updates: Sender<Update>,
}

/// Creates the typed bounded channels used by one application state owner.
#[must_use]
pub fn bounded_channels(capacity: NonZeroUsize) -> (AppHandle, AppChannels) {
    let (input_tx, input_rx) = async_channel::bounded(capacity.get());
    let (update_tx, update_rx) = async_channel::bounded(capacity.get());
    (
        AppHandle {
            inputs: input_tx,
            updates: update_rx,
        },
        AppChannels {
            inputs: input_rx,
            updates: update_tx,
        },
    )
}

/// Sole owner of mutable application state.
pub struct App {
    view: View,
}

impl App {
    /// Creates an application waiting for initial adapter data.
    #[must_use]
    pub fn new() -> Self {
        Self {
            view: View {
                connection: ConnectionState::Connecting,
                account_name: "Popgram".to_owned(),
                folders: Vec::new(),
                active_folder: 0,
                chats: Vec::new(),
                active_chat: None,
                messages: Vec::new(),
                active_message: None,
                focus: Focus::Chats,
                composer: ComposerView::default(),
                search: None,
                has_newer_messages: false,
                help_open: false,
                actions: Vec::new(),
            },
        }
    }

    /// Processes ordered input until every producer disconnects.
    pub async fn run(mut self, channels: AppChannels) -> Result<()> {
        self.refresh_actions();
        self.publish(&channels.updates, None).await?;
        while let Ok(input) = channels.inputs.recv().await {
            let effect = self.apply(input);
            self.refresh_actions();
            self.publish(&channels.updates, effect).await?;
        }
        Ok(())
    }

    fn apply(&mut self, input: Input) -> Option<Effect> {
        match input {
            Input::Adapter(AdapterEvent::Bootstrap(bootstrap)) => {
                self.view.connection = ConnectionState::Connected;
                self.view.account_name = bootstrap.account_name;
                self.view.folders = bootstrap.folders;
                self.view.chats = bootstrap.chats;
                self.view.messages = bootstrap.messages;
                self.view.active_chat = (!self.view.chats.is_empty()).then_some(0);
                self.view.active_message = self.view.messages.len().checked_sub(1);
                None
            }
            Input::Adapter(AdapterEvent::ConnectionChanged(connection)) => {
                self.view.connection = connection;
                None
            }
            Input::Adapter(AdapterEvent::MessageAdded(message)) => {
                let was_latest = self.at_latest();
                self.view.messages.push(message);
                if was_latest || self.view.focus == Focus::Composer {
                    self.view.active_message = self.view.messages.len().checked_sub(1);
                    self.view.has_newer_messages = false;
                } else {
                    self.view.has_newer_messages = true;
                }
                None
            }
            Input::Adapter(AdapterEvent::ChatLoaded { chat, messages }) => {
                if self.active_chat_id() == Some(chat) {
                    self.view.messages = messages;
                    self.view.active_message = self.view.messages.len().checked_sub(1);
                    self.view.has_newer_messages = false;
                }
                None
            }
            Input::Intent(intent) => self.apply_intent(intent),
        }
    }

    fn apply_intent(&mut self, intent: Intent) -> Option<Effect> {
        match intent {
            Intent::Insert(text) => {
                if let Some(search) = &mut self.view.search {
                    search.query.push_str(&text);
                } else if self.view.active_chat.is_some() {
                    self.view.focus = Focus::Composer;
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
            Intent::Newline => {
                if self.view.focus == Focus::Composer {
                    self.view.composer.text.push('\n');
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
            Action::FocusNext => {
                self.view.focus = match self.view.focus {
                    Focus::Folders => Focus::Chats,
                    Focus::Chats => Focus::Transcript,
                    Focus::Transcript | Focus::Search => Focus::Composer,
                    Focus::Composer => Focus::Folders,
                };
                None
            }
            Action::MoveUp => {
                self.move_active(false);
                None
            }
            Action::MoveDown => {
                self.move_active(true);
                None
            }
            Action::Open => {
                if let Some(chat) = self.active_chat_id() {
                    self.view.focus = Focus::Transcript;
                    return Some(Effect::LoadChat { chat });
                }
                None
            }
            Action::Compose => {
                if self.view.active_chat.is_some() {
                    self.view.focus = Focus::Composer;
                }
                None
            }
            Action::Reply => {
                self.view.composer.reply_to = self.active_message_id();
                if self.view.composer.reply_to.is_some() {
                    self.view.focus = Focus::Composer;
                }
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
                } else if self.view.search.take().is_some() {
                    self.view.focus = if self.view.active_chat.is_some() {
                        Focus::Transcript
                    } else {
                        Focus::Chats
                    };
                } else if self.view.composer.reply_to.take().is_some() {
                    self.view.focus = Focus::Composer;
                }
                None
            }
            Action::JumpEarliest => {
                self.view.active_message = (!self.view.messages.is_empty()).then_some(0);
                self.view.focus = Focus::Transcript;
                None
            }
            Action::JumpLatest => {
                self.view.active_message = self.view.messages.len().checked_sub(1);
                self.view.has_newer_messages = false;
                self.view.focus = Focus::Transcript;
                None
            }
            Action::Send => self.send_message(),
        }
    }

    fn send_message(&mut self) -> Option<Effect> {
        let chat_index = self.view.active_chat?;
        let text = self.view.composer.text.trim_end().to_owned();
        if text.is_empty() {
            return None;
        }
        let effect = Effect::SendMessage {
            chat: self.view.chats[chat_index].id,
            text,
            reply_to: self.view.composer.reply_to,
        };
        self.view.composer = ComposerView::default();
        self.view.focus = Focus::Transcript;
        Some(effect)
    }

    fn move_active(&mut self, forward: bool) {
        match self.view.focus {
            Focus::Folders => {
                self.view.active_folder = move_index(
                    Some(self.view.active_folder),
                    self.view.folders.len(),
                    forward,
                )
                .unwrap_or(0);
            }
            Focus::Chats => {
                self.view.active_chat =
                    move_index(self.view.active_chat, self.view.chats.len(), forward);
            }
            Focus::Transcript => {
                self.view.active_message =
                    move_index(self.view.active_message, self.view.messages.len(), forward);
                if self.at_latest() {
                    self.view.has_newer_messages = false;
                }
            }
            Focus::Composer | Focus::Search => {}
        }
    }

    fn active_message_id(&self) -> Option<MessageId> {
        self.view
            .active_message
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
        self.view.active_message == self.view.messages.len().checked_sub(1)
    }

    fn refresh_actions(&mut self) {
        let mut actions = vec![Action::Quit, Action::Help, Action::FocusNext];
        if self.view.help_open {
            self.view.actions = vec![Action::Quit, Action::Help, Action::Cancel];
            return;
        }
        match self.view.focus {
            Focus::Folders | Focus::Chats => {
                actions.extend([
                    Action::MoveUp,
                    Action::MoveDown,
                    Action::Open,
                    Action::Search,
                ]);
            }
            Focus::Transcript => {
                actions.extend([
                    Action::MoveUp,
                    Action::MoveDown,
                    Action::Compose,
                    Action::Reply,
                    Action::Search,
                    Action::JumpEarliest,
                    Action::JumpLatest,
                ]);
            }
            Focus::Composer => {
                actions.extend([Action::Send, Action::Cancel]);
            }
            Focus::Search => actions.push(Action::Cancel),
        }
        if self.view.connection == ConnectionState::ReconnectCooldown {
            actions.push(Action::Reconnect);
        }
        self.view.actions = actions;
    }

    async fn publish(&self, updates: &Sender<Update>, effect: Option<Effect>) -> Result<()> {
        updates
            .send(Update {
                view: self.view.clone(),
                effect,
            })
            .await
            .map_err(|_| Error::OutputClosed)
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
    use std::num::NonZeroUsize;

    use futures_lite::future;

    use super::{
        Action, AdapterEvent, App, Bootstrap, ChatId, ChatView, ConnectionState, DeliveryState,
        Effect, FolderView, Input, Intent, MessageDirection, MessageId, MessageView, SearchScope,
        bounded_channels,
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
                title: "Popgram".to_owned(),
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

    #[test]
    fn new_messages_do_not_snap_transcript_while_reading_older_history() {
        future::block_on(async {
            let capacity = NonZeroUsize::new(8).expect("fixture capacity should be positive");
            let (handle, channels) = bounded_channels(capacity);
            let drive = App::new().run(channels);
            let observe = async move {
                handle
                    .updates
                    .recv()
                    .await
                    .expect("initial view should arrive");
                handle
                    .inputs
                    .send(Input::Adapter(AdapterEvent::Bootstrap(bootstrap())))
                    .await
                    .expect("bootstrap should be accepted");
                handle
                    .updates
                    .recv()
                    .await
                    .expect("bootstrap view should arrive");
                handle
                    .inputs
                    .send(Input::Intent(Intent::Action(Action::Open)))
                    .await
                    .expect("open should be accepted");
                handle
                    .updates
                    .recv()
                    .await
                    .expect("transcript view should arrive");
                handle
                    .inputs
                    .send(Input::Intent(Intent::Action(Action::MoveUp)))
                    .await
                    .expect("navigation should be accepted");
                let older = handle
                    .updates
                    .recv()
                    .await
                    .expect("navigation view should arrive");
                assert_eq!(older.view.active_message, Some(1));

                handle
                    .inputs
                    .send(Input::Adapter(AdapterEvent::MessageAdded(MessageView {
                        id: MessageId(4),
                        sender: "Lin".to_owned(),
                        body: "new".to_owned(),
                        timestamp: "12:01".to_owned(),
                        direction: MessageDirection::Incoming,
                        delivery: DeliveryState::Sent,
                        reply_to: None,
                    })))
                    .await
                    .expect("new message should be accepted");
                let updated = handle
                    .updates
                    .recv()
                    .await
                    .expect("message view should arrive");
                assert_eq!(updated.view.active_message, Some(1));
                assert!(updated.view.has_newer_messages);
                drop(handle.inputs);
            };
            let (result, ()) = future::zip(drive, observe).await;
            result.expect("application should shut down cleanly");
        });
    }

    #[test]
    fn search_scope_and_reply_send_follow_current_context() {
        future::block_on(async {
            let capacity = NonZeroUsize::new(16).expect("fixture capacity should be positive");
            let (handle, channels) = bounded_channels(capacity);
            let drive = App::new().run(channels);
            let observe = async move {
                handle
                    .updates
                    .recv()
                    .await
                    .expect("initial view should arrive");
                handle
                    .inputs
                    .send(Input::Adapter(AdapterEvent::Bootstrap(bootstrap())))
                    .await
                    .expect("bootstrap should be accepted");
                handle
                    .updates
                    .recv()
                    .await
                    .expect("bootstrap view should arrive");

                handle
                    .inputs
                    .send(Input::Intent(Intent::Action(Action::Search)))
                    .await
                    .expect("search should be accepted");
                let search = handle
                    .updates
                    .recv()
                    .await
                    .expect("search view should arrive");
                assert_eq!(
                    search.view.search.expect("search should be open").scope,
                    SearchScope::Account
                );
                handle
                    .inputs
                    .send(Input::Intent(Intent::Action(Action::Cancel)))
                    .await
                    .expect("cancel should be accepted");
                handle
                    .updates
                    .recv()
                    .await
                    .expect("cancel view should arrive");
                handle
                    .inputs
                    .send(Input::Intent(Intent::Action(Action::Open)))
                    .await
                    .expect("open should be accepted");
                handle
                    .updates
                    .recv()
                    .await
                    .expect("transcript view should arrive");
                handle
                    .inputs
                    .send(Input::Intent(Intent::Action(Action::Reply)))
                    .await
                    .expect("reply should be accepted");
                handle
                    .updates
                    .recv()
                    .await
                    .expect("reply view should arrive");
                handle
                    .inputs
                    .send(Input::Intent(Intent::Insert("hello".to_owned())))
                    .await
                    .expect("draft should be accepted");
                handle
                    .updates
                    .recv()
                    .await
                    .expect("draft view should arrive");
                handle
                    .inputs
                    .send(Input::Intent(Intent::Action(Action::Send)))
                    .await
                    .expect("send should be accepted");
                let sent = handle
                    .updates
                    .recv()
                    .await
                    .expect("send effect should arrive");
                assert_eq!(
                    sent.effect,
                    Some(Effect::SendMessage {
                        chat: ChatId(10),
                        text: "hello".to_owned(),
                        reply_to: Some(MessageId(3)),
                    })
                );
                drop(handle.inputs);
            };
            let (result, ()) = future::zip(drive, observe).await;
            result.expect("application should shut down cleanly");
        });
    }

    #[test]
    fn reconnect_is_available_only_during_cooldown() {
        future::block_on(async {
            let capacity = NonZeroUsize::new(4).expect("fixture capacity should be positive");
            let (handle, channels) = bounded_channels(capacity);
            let drive = App::new().run(channels);
            let observe = async move {
                let initial = handle
                    .updates
                    .recv()
                    .await
                    .expect("initial view should arrive");
                assert!(!initial.view.actions.contains(&Action::Reconnect));
                handle
                    .inputs
                    .send(Input::Adapter(AdapterEvent::ConnectionChanged(
                        ConnectionState::ReconnectCooldown,
                    )))
                    .await
                    .expect("cooldown event should be accepted");
                let cooldown = handle
                    .updates
                    .recv()
                    .await
                    .expect("cooldown view should arrive");
                assert!(cooldown.view.actions.contains(&Action::Reconnect));
                handle
                    .inputs
                    .send(Input::Intent(Intent::Action(Action::Reconnect)))
                    .await
                    .expect("reconnect intent should be accepted");
                let reconnecting = handle
                    .updates
                    .recv()
                    .await
                    .expect("reconnect view should arrive");
                assert_eq!(reconnecting.view.connection, ConnectionState::Connecting);
                assert!(!reconnecting.view.actions.contains(&Action::Reconnect));
                assert_eq!(reconnecting.effect, Some(Effect::Reconnect));
                drop(handle.inputs);
            };
            let (result, ()) = future::zip(drive, observe).await;
            result.expect("application should shut down cleanly");
        });
    }
}
