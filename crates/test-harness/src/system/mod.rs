//! Hermetic application composition and synchronous test driver.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use intuigram::{Application, UpdateCommitter};
use intuigram_app::{AccountLifecycle, DownloadId};
use intuigram_store::AccountDatabase;
use intuigram_tui::{render_test_frame, resolve_test_event};
use tempfile::TempDir;

use super::error::{Error, Result};
use super::screen::{RenderedState, Screen, rows};
use super::telegram::TelegramScenario;
use super::trace::Trace;

mod assertions;
mod builder;
mod downloads;
mod effects;
mod input;
mod telegram_control;

pub use builder::TestSystemBuilder;
pub use input::{TestKey, key};
pub use telegram_control::TelegramControl;

pub struct TestSystem {
    application: Application,
    telegram: TelegramScenario,
    database: AccountDatabase,
    updates: UpdateCommitter,
    _root: TempDir,
    download_root: PathBuf,
    next_download_id: u64,
    next_update_pts: i32,
    downloaded_paths: Vec<PathBuf>,
    opened_links: Vec<String>,
    opened_downloads: Vec<(DownloadId, bool)>,
    account_lifecycle: Vec<AccountLifecycle>,
    terminal: (u16, u16),
    trace: Rc<RefCell<Trace>>,
    state: Rc<RefCell<RenderedState>>,
}

impl TestSystem {
    #[must_use]
    pub fn builder() -> TestSystemBuilder {
        TestSystemBuilder::default()
    }

    #[must_use]
    pub fn screen(&self) -> Screen {
        Screen::new(Rc::clone(&self.state), Rc::clone(&self.trace))
    }

    pub fn press(&mut self, key: TestKey) -> Result<()> {
        self.deliver_event(key.event())
    }

    pub fn type_text(&mut self, text: &str) -> Result<()> {
        for character in text.chars() {
            self.deliver_event(Event::Key(KeyEvent::new_with_kind(
                KeyCode::Char(character),
                KeyModifiers::NONE,
                KeyEventKind::Press,
            )))?;
        }
        Ok(())
    }

    pub fn paste(&mut self, text: impl Into<String>) -> Result<()> {
        self.deliver_event(Event::Paste(text.into()))
    }

    pub fn resize(&mut self, width: u16, height: u16) -> Result<()> {
        self.terminal = (width, height);
        self.deliver_event(Event::Resize(width, height))
    }

    pub fn focus(&mut self, focused: bool) -> Result<()> {
        self.deliver_event(if focused {
            Event::FocusGained
        } else {
            Event::FocusLost
        })
    }

    #[must_use]
    pub fn telegram(&mut self) -> TelegramControl<'_> {
        TelegramControl { system: self }
    }

    #[must_use]
    pub fn opened_links(&self) -> &[String] {
        &self.opened_links
    }

    #[must_use]
    pub fn downloaded_paths(&self) -> &[PathBuf] {
        &self.downloaded_paths
    }

    #[must_use]
    pub fn opened_downloads(&self) -> &[(DownloadId, bool)] {
        &self.opened_downloads
    }

    pub fn expect_account_lifecycle(&mut self, expected: AccountLifecycle) -> Result<()> {
        let actual = self.account_lifecycle.first().copied();
        if actual == Some(expected) {
            self.account_lifecycle.remove(0);
            Ok(())
        } else {
            Err(Error::Expectation {
                expectation: format!("Account lifecycle request {expected:?}"),
                actual: format!("{actual:?}"),
                artifact: self.trace.borrow().persist(),
            })
        }
    }

    pub fn expect_no_unhandled_work(&mut self) -> Result<()> {
        let mut pending = self.telegram.pending();
        if self.application.has_pending_effects() {
            pending.push("application adapter effects".to_owned());
        }
        if !self.account_lifecycle.is_empty() {
            pending.push("unobserved Account lifecycle requests".to_owned());
        }
        self.trace.borrow_mut().set_pending(pending.clone());
        if pending.is_empty() {
            Ok(())
        } else {
            Err(Error::PendingWork {
                work: pending.join(", "),
                artifact: self.trace.borrow().persist(),
            })
        }
    }

    fn deliver_event(&mut self, event: Event) -> Result<()> {
        self.trace
            .borrow_mut()
            .record("input", format!("{event:?}"), self.application.revision());
        let Some(ui_event) = resolve_test_event(self.application.view(), event.clone()) else {
            return Err(Error::UnavailableInput {
                event: format!("{event:?}"),
                artifact: self.trace.borrow().persist(),
            });
        };
        self.application.handle_ui(ui_event);
        self.render();
        self.drain_effects()
    }

    fn render(&mut self) {
        let frame = render_test_frame(self.application.view(), self.terminal.0, self.terminal.1);
        let screen_rows = rows(&frame.buffer);
        *self.state.borrow_mut() = RenderedState {
            view: self.application.view().clone(),
            buffer: frame.buffer,
            semantics: frame.semantics,
            revision: self.application.revision(),
        };
        let mut trace = self.trace.borrow_mut();
        trace.record(
            "render",
            "latest immutable view",
            self.application.revision(),
        );
        trace.update_screen(screen_rows);
    }
}
