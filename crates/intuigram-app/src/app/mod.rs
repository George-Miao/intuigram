/// Sole owner of mutable application state.
pub struct App {
    view: View,
    all_chats: Vec<ChatView>,
    muted_chats: HashSet<ChatId>,
    drafts: HashMap<HistoryKey, ComposerView>,
    histories: HashMap<HistoryKey, Vec<MessageView>>,
    pinned_histories: HashMap<ChatId, Vec<MessageView>>,
    projected_pin: bool,
    transcript_anchors: HashMap<HistoryKey, MessageId>,
    unread_boundaries: HashMap<HistoryKey, MessageId>,
    history_loads: HistoryLoads,
    media_preview_loads: MediaPreviewLoads,
    next_local_message_id: i64,
    pending_drafts: HashMap<MessageId, PendingDraft>,
    saved_poll_draft: Option<ComposerView>,
    pending_polls: HashMap<MessageId, PendingPoll>,
}

impl App {
    /// Creates an application waiting for initial adapter data.
    #[must_use]
    pub fn new() -> Self {
        let mut app = Self::empty();
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
                self.replace_bootstrap(bootstrap);
                if self.view.connection != ConnectionState::Connected {
                    return None;
                }
                self.queue_active_media_previews();
                self.request_next_media_preview()
                    .or_else(|| self.request_next_background_history())
            }
            Input::Adapter(AdapterEvent::ConnectionRestored(bootstrap)) => {
                self.merge_restored_connection(bootstrap);
                self.queue_active_media_previews();
                self.request_next_background_history()
                    .or_else(|| self.request_next_media_preview())
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
            Input::Adapter(AdapterEvent::ChatMuteChanged { chat, muted }) => {
                if muted {
                    self.muted_chats.insert(chat);
                } else {
                    self.muted_chats.remove(&chat);
                }
                None
            }
            Input::Adapter(
                event @ (AdapterEvent::FolderMembershipChanged { .. }
                | AdapterEvent::FolderOperationCompleted { .. }
                | AdapterEvent::FolderReconciled(_)
                | AdapterEvent::FolderReconciliationFailed(_)
                | AdapterEvent::FolderOperationFailed(_)),
            ) => self.apply_folder_adapter_event(event),
            Input::Adapter(
                event @ (AdapterEvent::RichMediaLibraryReady { .. }
                | AdapterEvent::RichMediaLibraryFailed(_)
                | AdapterEvent::RichMediaAcknowledged { .. }
                | AdapterEvent::RichMediaFailed { .. }),
            ) => {
                self.apply_rich_media_event(event);
                None
            }
            Input::Adapter(
                event @ (AdapterEvent::ScheduledMessagesReady { .. }
                | AdapterEvent::ScheduledOperationCompleted { .. }
                | AdapterEvent::ScheduledOperationFailed { .. }),
            ) => self.apply_scheduled_event(event),
            Input::Adapter(AdapterEvent::OperationFailed(reason)) => {
                self.view.notice = Some(reason);
                None
            }
            Input::Adapter(AdapterEvent::AccountLifecycleReady(_)) => None,
            Input::Adapter(AdapterEvent::OperationCompleted(message)) => {
                self.view.notice = Some(message);
                None
            }
            Input::Adapter(AdapterEvent::ChatDiscovered { chat }) => {
                if !self
                    .all_chats
                    .iter()
                    .any(|candidate| candidate.id == chat.id)
                {
                    let active = self.active_chat_id();
                    self.all_chats.push(chat);
                    self.refresh_folder_chats(active);
                }
                None
            }
            Input::Adapter(AdapterEvent::MessageAdded { chat, message }) => {
                self.apply_added_message(chat, *message)
            }
            Input::Adapter(AdapterEvent::MessageUpdated { chat, message }) => {
                self.replace_message(chat, *message);
                None
            }
            Input::Adapter(AdapterEvent::MessagesPinChanged { chat, ids, pinned }) => {
                self.reconcile_message_pins(chat, &ids, pinned)
            }
            Input::Adapter(AdapterEvent::MessageEditFailed {
                chat,
                message,
                text,
                attachments,
                reason,
            }) => {
                self.restore_failed_edit(chat, message, text, attachments);
                self.view.notice = Some(reason);
                None
            }
            Input::Adapter(AdapterEvent::MessagesDeleted { chat, ids }) => {
                self.delete_messages(chat, &ids);
                None
            }
            Input::Adapter(AdapterEvent::HistoryRead {
                chat,
                max_id,
                outgoing,
                unread,
            }) => {
                self.apply_read_state(chat, max_id, outgoing, unread);
                None
            }
            Input::Adapter(AdapterEvent::ChatArchiveChanged { chat, archived }) => {
                self.apply_folder_membership(chat, -1, archived)
            }
            Input::Adapter(AdapterEvent::ChatPinPermissionChanged {
                chat,
                can_pin_messages,
            }) => self.apply_chat_pin_permission(chat, can_pin_messages),
            Input::Adapter(AdapterEvent::ChatLoaded {
                chat,
                status,
                messages,
                pinned_messages,
            }) => {
                let key = HistoryKey { chat, thread: None };
                if let Some(status) = status {
                    self.apply_chat_status(chat, status);
                }
                self.store_loaded_pins(chat, pinned_messages);
                self.store_loaded_history(key, messages);
                if self.active_history_key() == Some(key) {
                    self.queue_active_media_previews();
                    self.defer_active_read();
                }
                self.complete_history_load(key, true)
            }
            Input::Adapter(AdapterEvent::HistoryLoadFailed {
                chat,
                thread_root,
                reason,
            }) => {
                let key = HistoryKey {
                    chat,
                    thread: thread_root,
                };
                if self.active_history_key() == Some(key) {
                    self.view.notice = Some(reason);
                }
                self.complete_history_load(key, false)
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
                self.store_loaded_history(key, messages);
                if self.active_history_key() == Some(key) {
                    self.queue_active_media_previews();
                    self.defer_active_read();
                }
                self.complete_history_load(key, true)
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
                        self.insert_composer_text(&text);
                    }
                    if self.view.composer.editing.is_some() && !attachments.is_empty() {
                        self.view.composer.attachments = attachments;
                    } else {
                        self.view.composer.attachments.extend(attachments);
                    }
                    self.view.focus = Focus::Composer;
                    self.draft_effect()
                } else {
                    let draft = self.drafts.entry(key).or_default();
                    if let Some(text) = text {
                        draft.text.push_str(&text);
                        draft.cursor = draft.text.len();
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
            Input::Adapter(AdapterEvent::AttachmentPathRequired { chat, thread_root }) => {
                let key = HistoryKey {
                    chat,
                    thread: thread_root,
                };
                if self.active_history_key() == Some(key) {
                    self.view.attachment_path = Some(AttachmentPathView {
                        path: String::new(),
                    });
                }
                None
            }
            Input::Adapter(AdapterEvent::MessageAcknowledged { chat, local_id }) => {
                self.update_delivery(chat, local_id, DeliveryState::Sent);
                self.pending_drafts.remove(&local_id);
                self.pending_polls.remove(&local_id);
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
                    .map(|pending| pending.composer)
                    .unwrap_or_else(|| {
                        let mut composer = ComposerView {
                            text: text.clone(),
                            ..ComposerView::default()
                        };
                        composer.cursor = composer.text.len();
                        composer
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
            Input::Adapter(AdapterEvent::PollFailed {
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
                let text = self
                    .pending_polls
                    .remove(&local_id)
                    .filter(|pending| pending.history == key)
                    .map_or(text, |pending| pending.text);
                if self.active_history_key() == Some(key) && self.view.composer.text.is_empty() {
                    self.saved_poll_draft = Some(self.view.composer.clone());
                    self.view.composer.cursor = text.len();
                    self.view.composer.text = text;
                    self.view.poll_composer = true;
                }
                self.view.notice = Some(reason);
                None
            }
            Input::Adapter(event @ AdapterEvent::TelegramLinkResolved { .. })
            | Input::Adapter(event @ AdapterEvent::DownloadReady { .. })
            | Input::Adapter(event @ AdapterEvent::MediaPreviewReady(_))
            | Input::Adapter(event @ AdapterEvent::MediaPreviewFailed { .. }) => {
                self.apply_link_media_event(event)
            }
            Input::Intent(intent) => self.apply_intent(intent),
        }
    }
}
use std::collections::{HashMap, HashSet};

use crate::domain::*;
use crate::history::{RefreshScope, reconcile_refresh};
use crate::protocol::*;

mod account_management;
mod action_availability;
mod action_menu;
mod actions;
mod bootstrap;
mod chat_reconciliation;
mod click_activation;
mod composer;
mod editing;
mod folder_management;
mod folder_navigation;
mod folder_reconciliation;
mod history_navigation;
mod history_reconciliation;
mod link_media;
mod media_preview;
mod message_selection;
mod messaging;
mod pinned;
mod poll_composer;
mod poll_vote;
mod rich_media;
mod scheduled;
mod state;
mod unread;

use action_availability::move_index;
use history_navigation::HistoryLoads;
use media_preview::{MediaPreviewLoads, PreviewKey};
use state::{HistoryKey, PendingDraft, PendingPoll};

const BACKGROUND_HISTORY_LIMIT: usize = 16;
