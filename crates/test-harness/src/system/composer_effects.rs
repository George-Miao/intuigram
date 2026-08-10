use intuigram_lib::{
    AdapterEvent, AttachmentId, AttachmentKind, AttachmentView, ChatId, MessageId, OutboxItemView,
    OutboxKey, OutboxStateView, TextEntity,
};
use intuigram_store::StoredDraft;
use snafu::ResultExt;

use super::TestSystem;
use crate::error::{Result, StoreSnafu};
use crate::telegram::{ObservedSavedSend, ObservedSend};

pub(super) struct ComposerSend {
    pub(super) chat: ChatId,
    pub(super) text: String,
    pub(super) entities: Vec<TextEntity>,
    pub(super) link_preview: bool,
    pub(super) reply_to: Option<MessageId>,
    pub(super) thread_root: Option<MessageId>,
    pub(super) saved_peer: Option<ChatId>,
    pub(super) local_id: MessageId,
}

impl TestSystem {
    pub(super) fn admit_composer_outbox(&mut self, chat: ChatId, local_id: MessageId) {
        self.next_outbox_key = self.next_outbox_key.saturating_add(1);
        let key = OutboxKey(self.next_outbox_key);
        self.outbox_items.insert(local_id, key);
        self.application
            .handle_adapter(AdapterEvent::OutboxChanged(OutboxItemView {
                key,
                chat,
                local_message: Some(local_id),
                state: OutboxStateView::Ready,
                retryable: false,
                available_at: None,
                expires_at: None,
                last_error: None,
            }));
    }

    pub(super) fn persist_composer_draft(
        &mut self,
        chat: ChatId,
        thread_root: Option<MessageId>,
        saved_peer: Option<ChatId>,
        text: String,
        reply_to: Option<MessageId>,
    ) -> Result<()> {
        self.database
            .save_draft(StoredDraft {
                chat_id: chat.0,
                thread_root: thread_root.map(|message| message.0),
                saved_peer: saved_peer.map(|peer| peer.0),
                text,
                reply_to: reply_to.map(|message| message.0),
                modified_at: 0,
            })
            .context(StoreSnafu)
    }

    pub(super) fn select_composer_attachment(
        &mut self,
        chat: ChatId,
        thread_root: Option<MessageId>,
        saved_peer: Option<ChatId>,
        path: String,
    ) {
        self.next_attachment_id = self.next_attachment_id.saturating_add(1);
        let id = AttachmentId(self.next_attachment_id);
        let name = std::path::Path::new(&path).file_name().map_or_else(
            || "attachment".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let kind = if name.ends_with(".png") || name.ends_with(".jpg") {
            AttachmentKind::Photo
        } else {
            AttachmentKind::File
        };
        self.attachment_names.insert(id, name.clone());
        self.application
            .handle_adapter(AdapterEvent::ClipboardReady {
                chat,
                thread_root,
                saved_peer,
                text: None,
                attachments: vec![AttachmentView { id, kind, name }],
            });
    }

    pub(super) fn hold_composer_send(&mut self, send: ComposerSend) -> Result<()> {
        let ComposerSend {
            chat,
            text,
            entities,
            link_preview,
            reply_to,
            thread_root,
            saved_peer,
            local_id,
        } = send;
        self.persist_composer_draft(chat, thread_root, saved_peer, String::new(), None)?;
        self.admit_composer_outbox(chat, local_id);
        let result = match saved_peer {
            Some(saved_peer) => self.telegram.hold_saved_send(ObservedSavedSend {
                chat,
                saved_peer,
                text,
                reply_to,
                thread_root,
                local_id,
            }),
            None => self.telegram.hold_send(ObservedSend {
                chat,
                text,
                entities,
                link_preview,
                reply_to,
                thread_root,
                local_id,
            }),
        };
        result.map_err(|error| self.scenario_error(error))
    }
}
