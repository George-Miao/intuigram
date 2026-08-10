use intuigram_lib::{ChatId, GeoPointView, MessageId, PlaceView};

use super::{ExpectedCommand, ScenarioMismatch, TelegramScenario};

impl TelegramScenario {
    #[must_use]
    pub fn expect_search_places(
        mut self,
        chat: i64,
        query: impl Into<String>,
        near: Option<GeoPointView>,
        places: impl IntoIterator<Item = PlaceView>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::SearchPlaces {
            chat: ChatId(chat),
            query: query.into(),
            near,
            places: places.into_iter().collect(),
        });
        self
    }

    #[must_use]
    pub fn expect_send_location(
        mut self,
        chat: i64,
        point: GeoPointView,
        reply_to: Option<i64>,
        thread_root: Option<i64>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::SendLocation {
            chat: ChatId(chat),
            point,
            reply_to: reply_to.map(MessageId),
            thread_root: thread_root.map(MessageId),
        });
        self
    }

    #[must_use]
    pub fn expect_send_venue(
        mut self,
        chat: i64,
        venue: PlaceView,
        reply_to: Option<i64>,
        thread_root: Option<i64>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::SendVenue {
            chat: ChatId(chat),
            venue,
            reply_to: reply_to.map(MessageId),
            thread_root: thread_root.map(MessageId),
        });
        self
    }

    pub fn search_places(
        &mut self,
        chat: ChatId,
        query: String,
        near: Option<GeoPointView>,
    ) -> Result<Vec<PlaceView>, ScenarioMismatch> {
        let observed = format!(
            "search for places {query:?} near {near:?} in Chat {}",
            chat.0
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::SearchPlaces {
                chat: expected_chat,
                query: expected_query,
                near: expected_near,
                places,
            } if expected_chat == chat && expected_query == query && expected_near == near => {
                Ok(places)
            }
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub fn send_location(
        &mut self,
        chat: ChatId,
        point: GeoPointView,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
    ) -> Result<(), ScenarioMismatch> {
        let observed = format!("send location {} to Chat {}", point.coordinates(), chat.0);
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::SendLocation {
                chat: expected_chat,
                point: expected_point,
                reply_to: expected_reply,
                thread_root: expected_thread,
            } if expected_chat == chat
                && expected_point == point
                && expected_reply == reply_to
                && expected_thread == thread_root =>
            {
                Ok(())
            }
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub fn send_venue(
        &mut self,
        chat: ChatId,
        venue: PlaceView,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
    ) -> Result<(), ScenarioMismatch> {
        let observed = format!("send venue {:?} to Chat {}", venue.title, chat.0);
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::SendVenue {
                chat: expected_chat,
                venue: expected_venue,
                reply_to: expected_reply,
                thread_root: expected_thread,
            } if expected_chat == chat
                && expected_venue == venue
                && expected_reply == reply_to
                && expected_thread == thread_root =>
            {
                Ok(())
            }
            expected => Err(mismatch(expected, observed)),
        }
    }
}

fn mismatch(expected: ExpectedCommand, observed: String) -> ScenarioMismatch {
    ScenarioMismatch {
        expected: expected.describe(),
        observed,
    }
}
