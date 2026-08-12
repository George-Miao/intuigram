use super::*;

mod completion;

impl App {
    pub(super) fn replace_outbox(&mut self, items: Vec<OutboxItemView>) {
        self.view.outbox = items;
        self.apply_outbox_projection();
    }

    pub(super) fn apply_outbox_changed(&mut self, item: OutboxItemView) {
        self.view
            .outbox
            .retain(|candidate| candidate.key != item.key);
        self.project_outbox_delivery(&item);
        self.view.outbox.push(item);
    }

    pub(super) fn outbox_message_actions(&self, message: MessageId) -> Vec<Action> {
        let Some(item) = self.outbox_for_message(message) else {
            return Vec::new();
        };
        match item.state {
            OutboxStateView::Ready | OutboxStateView::InFlight | OutboxStateView::Deferred => {
                vec![Action::CancelOutbox]
            }
            OutboxStateView::CancelRequested => Vec::new(),
            OutboxStateView::Failed if item.retryable => {
                vec![Action::RetryOutbox, Action::DismissOutbox]
            }
            OutboxStateView::Failed | OutboxStateView::Expired | OutboxStateView::Cancelled => {
                vec![Action::DismissOutbox]
            }
            OutboxStateView::Conflict | OutboxStateView::OutcomeUnknown => {
                vec![Action::ResolveOutbox, Action::DismissOutbox]
            }
        }
    }

    pub(super) fn resolve_active_outbox(&self, action: Action) -> Option<Effect> {
        let message = self.active_message_id()?;
        let item = self.outbox_for_message(message)?;
        if !self.outbox_message_actions(message).contains(&action) {
            return None;
        }
        let action = match (action, item.state) {
            (Action::CancelOutbox, _) => OutboxAction::Cancel,
            (Action::RetryOutbox, _) => OutboxAction::Retry,
            (Action::ResolveOutbox, OutboxStateView::Conflict) => OutboxAction::ResolveConflict,
            (Action::ResolveOutbox, OutboxStateView::OutcomeUnknown) => {
                OutboxAction::ResolveOutcomeUnknown
            }
            (Action::DismissOutbox, _) => OutboxAction::Dismiss,
            _ => return None,
        };
        Some(Effect::ResolveOutbox {
            item: item.key,
            action,
        })
    }

    pub(super) fn apply_outbox_projection(&mut self) {
        let items = self.view.outbox.clone();
        for item in &items {
            self.project_outbox_delivery(item);
        }
    }

    fn outbox_for_message(&self, message: MessageId) -> Option<&OutboxItemView> {
        self.view
            .outbox
            .iter()
            .find(|item| item.local_message == Some(message))
    }

    fn project_outbox_delivery(&mut self, item: &OutboxItemView) {
        let Some(message) = item.local_message else {
            return;
        };
        let delivery = match item.state {
            OutboxStateView::Ready
            | OutboxStateView::InFlight
            | OutboxStateView::Deferred
            | OutboxStateView::CancelRequested => DeliveryState::Pending,
            OutboxStateView::Failed
            | OutboxStateView::Conflict
            | OutboxStateView::OutcomeUnknown
            | OutboxStateView::Expired
            | OutboxStateView::Cancelled => DeliveryState::Failed,
        };
        self.update_delivery(item.chat, message, delivery);
    }
}
