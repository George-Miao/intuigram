use intuigram::encode_stored_message;
use intuigram_app::{AdapterEvent, ChatId, Effect, MessageView};
use snafu::ResultExt;

use super::TestSystem;
use super::telegram_control::block_on;
use crate::error::{Result, StoreSnafu};

impl TestSystem {
    pub(super) fn handle_specialized_effect(&mut self, effect: Effect) -> Result<()> {
        let (chat, updated) = match effect {
            Effect::RefreshSpecialized {
                chat,
                message,
                target,
            } => {
                let updated = self
                    .telegram
                    .refresh_specialized(chat, message.id, target)
                    .map_err(|error| self.scenario_error(error))?;
                (chat, updated)
            }
            Effect::ToggleTodoItem {
                chat,
                message,
                item,
                completed,
            } => {
                let updated = self
                    .telegram
                    .toggle_todo_item(chat, message.id, item, completed)
                    .map_err(|error| self.scenario_error(error))?;
                (chat, updated)
            }
            Effect::AppendTodoItem {
                chat,
                message,
                title,
            } => {
                let updated = self
                    .telegram
                    .append_todo_item(chat, message.id, title)
                    .map_err(|error| self.scenario_error(error))?;
                (chat, updated)
            }
            _ => unreachable!("the specialized-effect route accepts only specialized effects"),
        };
        self.persist_updated_message(chat, updated)
    }

    fn persist_updated_message(&mut self, chat: ChatId, updated: MessageView) -> Result<()> {
        let request = self
            .database
            .store()
            .save_messages(vec![encode_stored_message(chat, &updated)])
            .context(StoreSnafu)?;
        block_on(request).context(StoreSnafu)?;
        self.application
            .handle_adapter(AdapterEvent::MessageUpdated {
                chat,
                message: Box::new(updated),
            });
        Ok(())
    }
}
