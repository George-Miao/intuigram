use super::*;

impl App {
    pub(super) fn open_scheduled(&mut self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        if self.view.focus == Focus::Chats || self.view.connection != ConnectionState::Connected {
            return None;
        }
        self.view.scheduled = Some(ScheduledManagerView {
            chat,
            saved_peer: self.view.active_saved_peer,
            messages: Vec::new(),
            selected: 0,
            editor: None,
            confirmation: None,
            pending: true,
        });
        Some(Effect::LoadScheduledMessages {
            chat,
            saved_peer: self.view.active_saved_peer,
        })
    }

    pub(super) fn apply_scheduled_action(&mut self, action: Action) -> Option<Effect> {
        if self
            .view
            .scheduled
            .as_ref()
            .is_some_and(|manager| manager.pending)
        {
            return None;
        }
        match action {
            Action::MoveUp | Action::MoveDown => {
                self.move_scheduled(action == Action::MoveDown);
                None
            }
            Action::NewScheduled => {
                self.open_scheduled_editor(ScheduledEditorOperation::Create);
                None
            }
            Action::EditScheduled => {
                if let Some(message) = self.selected_scheduled().cloned() {
                    self.open_scheduled_editor(ScheduledEditorOperation::Edit(message.id));
                    if let Some(editor) = self
                        .view
                        .scheduled
                        .as_mut()
                        .and_then(|manager| manager.editor.as_mut())
                    {
                        editor.text = message.summary;
                    }
                }
                None
            }
            Action::RescheduleScheduled => {
                if let Some(message) = self.selected_scheduled().cloned() {
                    self.open_scheduled_editor(ScheduledEditorOperation::Reschedule(message.id));
                    if let Some(editor) = self
                        .view
                        .scheduled
                        .as_mut()
                        .and_then(|manager| manager.editor.as_mut())
                    {
                        editor.delivery = message.delivery.editable();
                    }
                }
                None
            }
            Action::DeleteScheduled | Action::SendScheduledNow => {
                if let Some(message) = self.selected_scheduled().map(|message| message.id)
                    && let Some(manager) = &mut self.view.scheduled
                {
                    manager.confirmation = Some(ScheduledConfirmationView {
                        message,
                        send_now: action == Action::SendScheduledNow,
                    });
                }
                None
            }
            Action::SaveScheduled => self.save_scheduled(),
            Action::ConfirmScheduled => self.confirm_scheduled(),
            Action::Cancel | Action::OpenScheduled => {
                let Some(manager) = &mut self.view.scheduled else {
                    return None;
                };
                if manager.confirmation.take().is_none() && manager.editor.take().is_none() {
                    self.view.scheduled = None;
                }
                None
            }
            _ => None,
        }
    }

    pub(super) fn insert_scheduled_text(&mut self, text: &str) -> bool {
        let Some(editor) = self
            .view
            .scheduled
            .as_mut()
            .and_then(|manager| manager.editor.as_mut())
        else {
            return self.view.scheduled.is_some();
        };
        if let Some(field) = scheduled_field(editor) {
            field.push_str(text);
        }
        true
    }

    pub(super) fn backspace_scheduled_text(&mut self) -> bool {
        let Some(editor) = self
            .view
            .scheduled
            .as_mut()
            .and_then(|manager| manager.editor.as_mut())
        else {
            return self.view.scheduled.is_some();
        };
        if let Some(field) = scheduled_field(editor) {
            field.pop();
        }
        true
    }

    pub(super) fn apply_scheduled_event(&mut self, event: AdapterEvent) -> Option<Effect> {
        let (chat, saved_peer, messages, notice) = match event {
            AdapterEvent::ScheduledMessagesReady {
                chat,
                saved_peer,
                messages,
            } => (chat, saved_peer, messages, None),
            AdapterEvent::ScheduledOperationCompleted {
                chat,
                saved_peer,
                messages,
                notice,
            } => (chat, saved_peer, messages, Some(notice)),
            AdapterEvent::ScheduledOperationFailed {
                chat,
                saved_peer,
                reason,
            } => {
                if let Some(manager) = &mut self.view.scheduled
                    && manager.chat == chat
                    && manager.saved_peer == saved_peer
                {
                    manager.pending = false;
                }
                self.view.notice = Some(reason);
                return None;
            }
            _ => return None,
        };
        if let Some(manager) = &mut self.view.scheduled
            && manager.chat == chat
            && manager.saved_peer == saved_peer
        {
            manager.messages = messages;
            manager.selected = manager
                .selected
                .min(manager.messages.len().saturating_sub(1));
            manager.editor = None;
            manager.confirmation = None;
            manager.pending = false;
            self.view.notice = notice;
        }
        None
    }

    fn selected_scheduled(&self) -> Option<&ScheduledMessageView> {
        let manager = self.view.scheduled.as_ref()?;
        manager.messages.get(manager.selected)
    }

    fn open_scheduled_editor(&mut self, operation: ScheduledEditorOperation) {
        if let Some(manager) = &mut self.view.scheduled {
            manager.editor = Some(ScheduledEditorView {
                operation,
                text: String::new(),
                delivery: String::new(),
                selected: 0,
            });
        }
    }

    fn move_scheduled(&mut self, down: bool) {
        let Some(manager) = &mut self.view.scheduled else {
            return;
        };
        if let Some(editor) = &mut manager.editor {
            let count =
                usize::from(matches!(editor.operation, ScheduledEditorOperation::Create)) + 1;
            editor.selected = move_index(Some(editor.selected), count, down).unwrap_or(0);
        } else {
            manager.selected =
                move_index(Some(manager.selected), manager.messages.len(), down).unwrap_or(0);
        }
    }

    fn save_scheduled(&mut self) -> Option<Effect> {
        let manager = self.view.scheduled.as_ref()?;
        let editor = manager.editor.as_ref()?;
        let request = match editor.operation {
            ScheduledEditorOperation::Create => ScheduledRequest::Create {
                delivery: ScheduledDeliveryView::parse(&editor.delivery)?,
                text: nonempty(&editor.text)?,
            },
            ScheduledEditorOperation::Edit(message) => ScheduledRequest::Edit {
                message,
                text: nonempty(&editor.text)?,
            },
            ScheduledEditorOperation::Reschedule(message) => ScheduledRequest::Reschedule {
                message,
                delivery: ScheduledDeliveryView::parse(&editor.delivery)?,
            },
        };
        self.begin_scheduled_operation(request)
    }

    fn confirm_scheduled(&mut self) -> Option<Effect> {
        let manager = self.view.scheduled.as_ref()?;
        let confirmation = manager.confirmation?;
        let request = if confirmation.send_now {
            ScheduledRequest::SendNow {
                message: confirmation.message,
            }
        } else {
            ScheduledRequest::Delete {
                message: confirmation.message,
            }
        };
        self.begin_scheduled_operation(request)
    }

    fn begin_scheduled_operation(&mut self, request: ScheduledRequest) -> Option<Effect> {
        let manager = self.view.scheduled.as_mut()?;
        manager.pending = true;
        manager.editor = None;
        manager.confirmation = None;
        Some(Effect::ScheduledOperation {
            chat: manager.chat,
            saved_peer: manager.saved_peer,
            request,
        })
    }
}

fn scheduled_field(editor: &mut ScheduledEditorView) -> Option<&mut String> {
    match (editor.operation, editor.selected) {
        (ScheduledEditorOperation::Create, 0) | (ScheduledEditorOperation::Edit(_), 0) => {
            Some(&mut editor.text)
        }
        (ScheduledEditorOperation::Create, 1) | (ScheduledEditorOperation::Reschedule(_), 0) => {
            Some(&mut editor.delivery)
        }
        _ => None,
    }
}

fn nonempty(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}
