impl App {
    pub(super) fn send_message(&mut self) -> Option<Effect> {
        if self.view.composer.editing.is_some() {
            return self.save_edit();
        }
        let chat_index = self.view.active_chat?;
        let chat = self.view.chats.get(chat_index)?.id;
        let draft_text = self.view.composer.text.trim_end();
        if draft_text.is_empty() && self.view.composer.attachments.is_empty() {
            return None;
        }
        let formatted = format_markdown(draft_text);
        self.next_local_message_id = self.next_local_message_id.saturating_sub(1);
        let local_id = MessageId(self.next_local_message_id);
        let key = self.active_history_key()?;
        self.pending_drafts.insert(
            local_id,
            PendingDraft {
                history: key,
                composer: self.view.composer.clone(),
            },
        );
        let pending = MessageView {
            id: local_id,
            sender: "You".to_owned(),
            body: formatted.text.clone(),
            timestamp: "now".to_owned(),
            direction: MessageDirection::Outgoing,
            delivery: DeliveryState::Pending,
            reply_to: self.view.composer.reply_to,
            details: MessageDetails {
                entities: formatted.entities.clone(),
                thread_root: self.view.active_thread,
                ..MessageDetails::default()
            },
        };
        self.histories.entry(key).or_default().push(pending);
        self.refresh_active_history();
        let effect = Effect::SendMessage {
            chat,
            text: formatted.text,
            entities: formatted.entities,
            link_preview: true,
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

    pub(super) fn draft_effect(&self) -> Option<Effect> {
        let key = self.active_history_key()?;
        (self.view.focus == Focus::Composer
            && self.view.composer.editing.is_none()
            && !self.view.poll_composer)
            .then(|| Effect::SaveDraft {
                chat: key.chat,
                thread_root: key.thread,
                text: self.view.composer.text.clone(),
                reply_to: self.view.composer.reply_to,
            })
    }

    pub(super) fn update_delivery(
        &mut self,
        chat: ChatId,
        message: MessageId,
        delivery: DeliveryState,
    ) {
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

    pub(super) fn reconcile_pending_message(
        &mut self,
        chat: ChatId,
        message: &MessageView,
    ) -> bool {
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
}
use super::*;
