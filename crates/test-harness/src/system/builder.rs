//! Isolated behavior-system construction.

use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use intuigram::{Application, UpdateCommitter, bootstrap_sync_batch};
use intuigram_store::{AccountDatabase, AccountId, StoreLayout, StoredDraft, SyncCursor};
use intuigram_tui::render_test_frame;
use snafu::ResultExt;

use super::TestSystem;
use crate::error::{CreateRootsSnafu, Error, Result, StoreSnafu};
use crate::screen::RenderedState;
use crate::telegram::TelegramScenario;
use crate::trace::Trace;

pub struct TestSystemBuilder {
    name: String,
    terminal: (u16, u16),
    virtual_time: String,
    seed: u64,
    telegram: Option<TelegramScenario>,
}

impl TestSystemBuilder {
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    #[must_use]
    pub fn terminal(mut self, width: u16, height: u16) -> Self {
        self.terminal = (width, height);
        self
    }

    #[must_use]
    pub fn time(mut self, time: impl Into<String>) -> Self {
        self.virtual_time = time.into();
        self
    }

    #[must_use]
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    #[must_use]
    pub fn telegram(mut self, telegram: TelegramScenario) -> Self {
        self.telegram = Some(telegram);
        self
    }

    pub fn start(self) -> Result<TestSystem> {
        let mut telegram = self.telegram.ok_or(Error::MissingTelegramScenario)?;
        let bootstrap = telegram
            .take_bootstrap()
            .ok_or(Error::MissingTelegramScenario)?;
        let root = tempfile::tempdir().context(CreateRootsSnafu)?;
        for directory in ["config", "data", "cache", "downloads"] {
            fs::create_dir(root.path().join(directory)).context(CreateRootsSnafu)?;
        }
        let layout = StoreLayout::new(root.path().join("data"));
        let download_root = root.path().join("downloads");
        let pending = AccountDatabase::begin_login(&layout).context(StoreSnafu)?;
        let account = AccountId::new(1).expect("the fixed behavior-test Account ID is positive");
        let database = pending.finish_login(&layout, account).context(StoreSnafu)?;
        let cursor = SyncCursor {
            scope: "account".to_owned(),
            ..SyncCursor::default()
        };
        database
            .commit_sync(bootstrap_sync_batch(&bootstrap, [cursor.clone()]))
            .context(StoreSnafu)?;
        for draft in &bootstrap.drafts {
            database
                .save_draft(StoredDraft {
                    chat_id: draft.chat.0,
                    thread_root: draft.thread_root.map(|message| message.0),
                    saved_peer: draft.saved_peer.map(|peer| peer.0),
                    text: draft.text.clone(),
                    reply_to: draft.reply_to.map(|message| message.0),
                    modified_at: 0,
                })
                .context(StoreSnafu)?;
        }
        let updates = UpdateCommitter::new(
            database.store(),
            [cursor],
            bootstrap.chats.iter().map(|chat| chat.id),
        );
        let application = Application::new(bootstrap);
        let trace = Rc::new(RefCell::new(Trace::new(
            self.name,
            self.virtual_time,
            self.seed,
            self.terminal,
        )));
        let frame = render_test_frame(application.view(), self.terminal.0, self.terminal.1);
        let state = Rc::new(RefCell::new(RenderedState {
            view: application.view().clone(),
            buffer: frame.buffer,
            semantics: frame.semantics,
            revision: application.revision(),
        }));
        trace.borrow_mut().record(
            "start",
            "fresh isolated application",
            application.revision(),
        );

        let mut system = TestSystem {
            application,
            telegram,
            database,
            updates,
            _root: root,
            download_root,
            next_download_id: 0,
            next_attachment_id: 0,
            attachment_names: std::collections::HashMap::new(),
            next_update_pts: 0,
            downloaded_paths: Vec::new(),
            opened_links: Vec::new(),
            opened_downloads: Vec::new(),
            notifications: Vec::new(),
            account_lifecycle: Vec::new(),
            scheduled_messages: std::collections::HashMap::new(),
            next_scheduled_id: 0,
            terminal: self.terminal,
            trace,
            state,
        };
        system.render();
        system.drain_effects()?;
        Ok(system)
    }
}

impl Default for TestSystemBuilder {
    fn default() -> Self {
        Self {
            name: "behavior".to_owned(),
            terminal: (100, 24),
            virtual_time: "2026-08-03T12:00:00Z".to_owned(),
            seed: 0x1_17_01_6A,
            telegram: None,
        }
    }
}
