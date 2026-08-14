use super::composer::append_attachments;
use super::*;

impl App {
    pub(super) fn apply(&mut self, input: Input) -> Option<Effect> {
        match input {
            Input::Adapter(AdapterEvent::Bootstrap(bootstrap)) => {
                self.replace_bootstrap(bootstrap);
                if self.view.connection != ConnectionState::Connected {
                    return None;
                }
                self.queue_all_offline_media();
                self.queue_active_media_previews();
                self.queue_visible_avatars();
                self.request_next_offline_media()
                    .or_else(|| self.request_next_small_media())
                    .or_else(|| self.request_next_background_history())
            }
            Input::Adapter(AdapterEvent::ConnectionRestored(bootstrap)) => {
                self.merge_restored_connection(bootstrap);
                self.queue_all_offline_media();
                self.queue_active_media_previews();
                self.queue_visible_avatars();
                self.request_next_offline_media()
                    .or_else(|| self.request_next_background_history())
                    .or_else(|| self.request_next_small_media())
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
            Input::Adapter(event @ AdapterEvent::TopicsLoaded(_))
            | Input::Adapter(event @ AdapterEvent::TopicsLoadFailed(_))
            | Input::Adapter(event @ AdapterEvent::ChatTopicsChanged(_)) => {
                self.apply_topic_event(event)
            }
            Input::Adapter(event @ AdapterEvent::SavedDialogsLoaded(_))
            | Input::Adapter(event @ AdapterEvent::SavedDialogsLoadFailed(_)) => {
                self.apply_saved_dialog_event(event)
            }
            Input::Adapter(
                event @ (AdapterEvent::ChatMediaOfflineChanged(_)
                | AdapterEvent::ChatMediaOfflineFailed(_)
                | AdapterEvent::MediaCachedOffline(_)
                | AdapterEvent::MediaCacheOfflineFailed(_)),
            ) => self.apply_offline_media_event(event),
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
                | AdapterEvent::PlaceSearchReady { .. }
                | AdapterEvent::PlaceSearchFailed { .. }
                | AdapterEvent::RichMediaAcknowledged { .. }
                | AdapterEvent::RichMediaFailed { .. }),
            ) => {
                self.apply_rich_media_event(event);
                None
            }
            Input::Adapter(
                event @ (AdapterEvent::ScheduledMessagesReady { .. }
                | AdapterEvent::ScheduledOperationCompleted { .. }
                | AdapterEvent::ScheduledOperationAcknowledged { .. }
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
                let effect = self.apply_added_message(chat, *message);
                self.queue_offline_media(chat);
                if self.active_chat_id() == Some(chat) {
                    self.queue_active_media_previews();
                }
                effect.or_else(|| self.request_next_offline_media())
            }
            Input::Adapter(AdapterEvent::MessageUpdated { chat, message }) => {
                self.replace_message(chat, *message);
                None
            }
            Input::Adapter(AdapterEvent::PaidMediaItemsUpdated {
                chat,
                message,
                items,
            }) => self.apply_paid_media_items(chat, message, items),
            Input::Adapter(AdapterEvent::AvatarChanged { peer, id }) => {
                self.update_avatar(peer, id)
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
                saved_peer,
                max_id,
                outgoing,
                unread,
            }) => {
                self.apply_read_state(chat, saved_peer, max_id, outgoing, unread);
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
                let key = HistoryKey::root(chat);
                if let Some(status) = status {
                    self.apply_chat_status(chat, status);
                }
                self.store_loaded_pins(chat, pinned_messages);
                self.store_loaded_history(key, messages);
                self.queue_offline_media(chat);
                if self.active_history_key() == Some(key) {
                    self.queue_active_media_previews();
                    self.queue_visible_avatars();
                    self.defer_active_read();
                }
                self.request_next_offline_media()
                    .or_else(|| self.complete_history_load(key, true))
            }
            Input::Adapter(AdapterEvent::HistoryLoadFailed {
                chat,
                thread_root,
                saved_peer,
                reason,
            }) => {
                let key = HistoryKey::scoped(chat, thread_root, saved_peer);
                if self.active_history_key() == Some(key) {
                    self.view.notice = Some(reason);
                }
                self.complete_history_load(key, false)
            }
            Input::Adapter(AdapterEvent::ThreadLoaded {
                chat,
                root,
                saved_peer,
                messages,
            }) => {
                let key = HistoryKey::scoped(chat, Some(root), saved_peer);
                self.store_loaded_history(key, messages);
                self.queue_offline_media(chat);
                if self.active_history_key() == Some(key) {
                    self.queue_active_media_previews();
                    self.queue_visible_avatars();
                    self.defer_active_read();
                }
                self.request_next_offline_media()
                    .or_else(|| self.complete_history_load(key, true))
            }
            Input::Adapter(event @ AdapterEvent::SavedHistoryLoaded { .. })
            | Input::Adapter(event @ AdapterEvent::SavedHistoryLoadFailed { .. }) => {
                self.apply_saved_history_event(event)
            }
            Input::Adapter(AdapterEvent::ClipboardReady {
                chat,
                thread_root,
                saved_peer,
                text,
                attachments,
            }) => {
                let key = HistoryKey::scoped(chat, thread_root, saved_peer);
                if self.active_history_key() == Some(key) {
                    if let Some(text) = text {
                        self.insert_composer_text(&text);
                    }
                    let replace = self.view.composer.editing.is_some();
                    append_attachments(&mut self.view.composer.attachments, attachments, replace);
                    self.view.focus = Focus::Composer;
                    self.draft_effect()
                } else {
                    let draft = self.drafts.entry(key).or_default();
                    if let Some(text) = text {
                        draft.text.push_str(&text);
                        draft.cursor = draft.text.len();
                    }
                    append_attachments(&mut draft.attachments, attachments, false);
                    Some(Effect::SaveDraft {
                        chat: key.chat,
                        thread_root: key.thread,
                        saved_peer: key.saved_peer,
                        text: draft.text.clone(),
                        reply_to: draft.reply_to,
                    })
                }
            }
            Input::Adapter(AdapterEvent::AttachmentPathRequired {
                chat,
                thread_root,
                saved_peer,
            }) => {
                let key = HistoryKey::scoped(chat, thread_root, saved_peer);
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
            Input::Adapter(AdapterEvent::MessageEditAcknowledged {
                chat,
                message,
                text,
                entities,
            }) => {
                self.apply_edit_acknowledgement(chat, message, text, entities);
                None
            }
            Input::Adapter(AdapterEvent::MessageMediaUpdated {
                chat,
                message,
                media,
            }) => {
                self.apply_message_media(chat, message, media);
                None
            }
            Input::Adapter(AdapterEvent::OutboxChanged(item)) => {
                self.apply_outbox_changed(item);
                None
            }
            Input::Adapter(AdapterEvent::OutboxRemoved { item }) => {
                self.view.outbox.retain(|candidate| candidate.key != item);
                None
            }
            Input::Adapter(AdapterEvent::MessageFailed {
                chat,
                local_id,
                thread_root,
                saved_peer,
                text,
                reason,
            }) => {
                self.update_delivery(chat, local_id, DeliveryState::Failed);
                let key = HistoryKey::scoped(chat, thread_root, saved_peer);
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
                    saved_peer,
                    text: draft_text,
                    reply_to: draft_reply_to,
                })
            }
            Input::Adapter(AdapterEvent::PollFailed {
                chat,
                local_id,
                thread_root,
                saved_peer,
                text,
                reason,
            }) => {
                self.update_delivery(chat, local_id, DeliveryState::Failed);
                let key = HistoryKey::scoped(chat, thread_root, saved_peer);
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
            | Input::Adapter(event @ AdapterEvent::MediaPreviewFailed { .. })
            | Input::Adapter(event @ AdapterEvent::AvatarReady(_))
            | Input::Adapter(event @ AdapterEvent::AvatarFailed { .. }) => {
                self.apply_link_media_event(event)
            }
            Input::Intent(intent) => self.apply_intent(intent),
            Input::EffectAccepted(EffectAdmission::SmallMedia) => self.request_next_small_media(),
            Input::EffectAccepted(EffectAdmission::Notification | EffectAdmission::ReadState) => {
                self.request_next_offline_media()
                    .or_else(|| self.request_next_small_media())
            }
            Input::ConfigureSmallMediaCapacity(capacity) => {
                self.small_media_capacity = capacity.max(1);
                None
            }
        }
    }
}
