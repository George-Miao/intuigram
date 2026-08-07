//! Durable-state assertions for behavior scenarios.

use intuigram_store::SyncCursor;
use snafu::ResultExt;

use super::TestSystem;
use crate::error::{Error, Result, StoreSnafu};

impl TestSystem {
    /// Requires one Message to be absent from durable storage.
    pub fn expect_no_durable_message(&self, chat: i64, id: i64) -> Result<()> {
        let cached = self.database.cached_account().context(StoreSnafu)?;
        if cached
            .messages
            .iter()
            .any(|message| message.chat_id == chat && message.id == id)
        {
            Err(Error::Expectation {
                expectation: format!("durable Message {id} is absent from Chat {chat}"),
                actual: "Message remains stored".to_owned(),
                artifact: self.trace.borrow().persist(),
            })
        } else {
            Ok(())
        }
    }

    /// Requires the root Chat Draft to be durably stored.
    pub fn expect_saved_draft(&self, chat: i64, text: &str) -> Result<()> {
        self.expect_saved_draft_for(chat, None, text)
    }

    /// Requires one Thread Draft to be durably stored independently.
    pub fn expect_saved_thread_draft(&self, chat: i64, root: i64, text: &str) -> Result<()> {
        self.expect_saved_draft_for(chat, Some(root), text)
    }

    fn expect_saved_draft_for(&self, chat: i64, root: Option<i64>, text: &str) -> Result<()> {
        let cached = self.database.cached_account().context(StoreSnafu)?;
        let actual = cached
            .drafts
            .iter()
            .find(|draft| draft.chat_id == chat && draft.thread_root == root)
            .map(|draft| draft.text.as_str());
        if actual == Some(text) {
            Ok(())
        } else {
            Err(Error::Expectation {
                expectation: format!("durable Draft for Chat {chat}, Thread {root:?} is {text:?}"),
                actual: format!("{actual:?}"),
                artifact: self.trace.borrow().persist(),
            })
        }
    }

    /// Requires one normalized Message to be durably stored.
    pub fn expect_durable_message(&self, chat: i64, id: i64, body: &str) -> Result<()> {
        self.expect_durable_message_in(chat, id, None, body)
    }

    /// Requires one Message to be durable in a particular Thread.
    pub fn expect_durable_thread_message(
        &self,
        chat: i64,
        id: i64,
        root: i64,
        body: &str,
    ) -> Result<()> {
        self.expect_durable_message_in(chat, id, Some(root), body)
    }

    fn expect_durable_message_in(
        &self,
        chat: i64,
        id: i64,
        root: Option<i64>,
        body: &str,
    ) -> Result<()> {
        let cached = self.database.cached_account().context(StoreSnafu)?;
        let actual = cached
            .messages
            .iter()
            .find(|message| message.chat_id == chat && message.id == id)
            .map(|message| (message.thread_root, message.body.as_str()));
        if actual == Some((root, body)) {
            Ok(())
        } else {
            Err(Error::Expectation {
                expectation: format!(
                    "durable Message {id} in Chat {chat}, Thread {root:?} has body {body:?}"
                ),
                actual: format!("{actual:?}"),
                artifact: self.trace.borrow().persist(),
            })
        }
    }

    /// Requires the complete Account synchronization cursor to be durable.
    pub fn expect_sync_cursor(
        &self,
        scope: &str,
        pts: i32,
        qts: i32,
        date: i32,
        seq: i32,
    ) -> Result<()> {
        let cached = self.database.cached_account().context(StoreSnafu)?;
        let expected = SyncCursor {
            scope: scope.to_owned(),
            pts,
            qts,
            date,
            seq,
        };
        if cached.cursors.iter().any(|cursor| cursor == &expected) {
            Ok(())
        } else {
            Err(Error::Expectation {
                expectation: format!("durable synchronization cursor is {expected:?}"),
                actual: format!("{:?}", cached.cursors),
                artifact: self.trace.borrow().persist(),
            })
        }
    }
}
