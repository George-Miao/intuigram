use intuigram_app::{AdapterEvent, ChatId, TopicListView};

use super::TestSystem;
use crate::error::Result;

impl TestSystem {
    pub(super) fn handle_topic_load(&mut self, chat: ChatId) -> Result<()> {
        let topics = self
            .telegram
            .load_topics(chat)
            .map_err(|error| self.scenario_error(error))?;
        self.application
            .handle_adapter(AdapterEvent::TopicsLoaded(TopicListView { chat, topics }));
        Ok(())
    }
}
