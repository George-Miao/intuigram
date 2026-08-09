use super::*;

impl App {
    pub(super) fn replace_topic_lists(&mut self, lists: Vec<TopicListView>) {
        self.topic_lists = lists
            .into_iter()
            .map(|list| (list.chat, list.topics))
            .collect();
    }

    pub(super) fn active_topic_id(&self) -> Option<TopicId> {
        self.view
            .active_topic
            .and_then(|index| self.view.topics.get(index))
            .map(|topic| topic.id)
    }

    pub(super) fn restore_reconnected_topics(&mut self, active: Option<TopicId>) {
        self.restore_active_topics();
        if let Some(active) = active {
            self.view.active_topic = self.view.topics.iter().position(|topic| topic.id == active);
        } else if self.view.active_thread.is_some() {
            self.view.active_topic = None;
        }
    }

    pub(super) fn apply_topic_event(&mut self, event: AdapterEvent) -> Option<Effect> {
        match event {
            AdapterEvent::TopicsLoaded(list) => self.apply_loaded_topics(list.chat, list.topics),
            AdapterEvent::TopicsLoadFailed(failure) => {
                if self.active_chat_id() == Some(failure.chat) {
                    self.view.topics_loading = false;
                    self.view.notice = Some(failure.reason);
                }
            }
            AdapterEvent::ChatTopicsChanged(availability) => {
                for chat in self
                    .all_chats
                    .iter_mut()
                    .chain(self.view.chats.iter_mut())
                    .filter(|chat| chat.id == availability.chat)
                {
                    chat.has_topics = availability.has_topics;
                }
            }
            _ => unreachable!("only Topic adapter events are routed here"),
        }
        None
    }

    pub(super) fn active_chat_has_topics(&self) -> bool {
        self.view
            .active_chat
            .and_then(|index| self.view.chats.get(index))
            .is_some_and(|chat| chat.has_topics)
    }

    pub(super) fn restore_active_topics(&mut self) {
        self.view.topics = self
            .active_chat_id()
            .and_then(|chat| self.topic_lists.get(&chat))
            .cloned()
            .unwrap_or_default();
        self.view.active_topic = (!self.view.topics.is_empty()).then_some(0);
        self.view.topics_loading = false;
    }

    pub(super) fn open_topics(&mut self, chat: ChatId) -> Option<Effect> {
        self.restore_active_topics();
        self.view.focus = Focus::Topics;
        self.view.active_thread = None;
        self.view.topics_loading = true;
        Some(Effect::LoadTopics(chat))
    }

    pub(super) fn apply_loaded_topics(&mut self, chat: ChatId, topics: Vec<TopicView>) {
        let selected = (self.active_chat_id() == Some(chat))
            .then(|| {
                self.view
                    .active_topic
                    .and_then(|index| self.view.topics.get(index))
                    .map(|topic| topic.id)
            })
            .flatten();
        for topic in &topics {
            if let Some(draft) = &topic.draft {
                self.drafts
                    .entry(HistoryKey {
                        chat,
                        thread: Some(topic.id.root_message()),
                    })
                    .or_insert_with(|| ComposerView {
                        text: draft.text.clone(),
                        cursor: draft.text.len(),
                        reply_to: draft.reply_to,
                        editing: None,
                        attachments: Vec::new(),
                    });
            }
        }
        self.topic_lists.insert(chat, topics);
        if self.active_chat_id() == Some(chat) {
            self.view.topics = self.topic_lists.get(&chat).cloned().unwrap_or_default();
            self.view.active_topic = selected
                .and_then(|selected| {
                    self.view
                        .topics
                        .iter()
                        .position(|topic| topic.id == selected)
                })
                .or_else(|| (!self.view.topics.is_empty()).then_some(0));
            self.view.topics_loading = false;
        }
    }

    pub(super) fn move_topic(&mut self, forward: bool) {
        self.view.active_topic =
            move_index(self.view.active_topic, self.view.topics.len(), forward);
    }

    pub(super) fn open_active_topic(&mut self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        let root = self
            .view
            .active_topic
            .and_then(|index| self.view.topics.get(index))?
            .id
            .root_message();
        self.save_active_draft();
        self.clear_message_selection();
        self.view.active_thread = Some(root);
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.restore_active_draft();
        self.refresh_active_history();
        self.view.focus = Focus::Composer;
        self.request_history_load(HistoryKey {
            chat,
            thread: Some(root),
        })
    }

    pub(super) fn leave_topic(&mut self) {
        self.save_active_draft();
        self.save_transcript_anchor();
        self.clear_message_selection();
        self.view.active_thread = None;
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.view.messages.clear();
        self.view.pinned_messages.clear();
        self.view.composer = ComposerView::default();
        self.view.focus = Focus::Topics;
    }
}
