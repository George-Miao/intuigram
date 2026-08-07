use std::cell::RefCell;
use std::rc::Rc;

use intuigram_app::{Action, ConnectionState, DeliveryState, MessageId, View};
use intuigram_tui::{SemanticNode, SemanticRole};
use ratatui::buffer::Buffer;

use crate::error::{Error, Result};
use crate::trace::Trace;

mod chrome;
mod message;

pub(crate) use chrome::rows;
pub use chrome::*;
pub use message::*;

#[derive(Clone, Debug)]
pub(crate) struct RenderedState {
    pub view: View,
    pub buffer: Buffer,
    pub semantics: Vec<SemanticNode>,
    pub revision: u64,
}

#[derive(Clone)]
pub struct Screen {
    state: Rc<RefCell<RenderedState>>,
    trace: Rc<RefCell<Trace>>,
}

impl Screen {
    pub(crate) fn new(state: Rc<RefCell<RenderedState>>, trace: Rc<RefCell<Trace>>) -> Self {
        Self { state, trace }
    }

    #[must_use]
    pub fn chat(&self, title: impl Into<String>) -> ChatLocator {
        ChatLocator {
            state: Rc::clone(&self.state),
            trace: Rc::clone(&self.trace),
            title: title.into(),
        }
    }

    #[must_use]
    pub fn message(&self, id: i64) -> MessageLocator {
        MessageLocator {
            state: Rc::clone(&self.state),
            trace: Rc::clone(&self.trace),
            query: MessageQuery::Id(MessageId(id)),
        }
    }

    #[must_use]
    pub fn message_text(&self, text: impl Into<String>) -> MessageLocator {
        MessageLocator {
            state: Rc::clone(&self.state),
            trace: Rc::clone(&self.trace),
            query: MessageQuery::Text(text.into()),
        }
    }

    /// Locates one Media Card by its user-facing title.
    #[must_use]
    pub fn media_card(&self, title: impl Into<String>) -> MediaCardLocator {
        MediaCardLocator {
            state: Rc::clone(&self.state),
            trace: Rc::clone(&self.trace),
            title: title.into(),
        }
    }

    #[must_use]
    pub fn composer(&self) -> ComposerLocator {
        ComposerLocator {
            state: Rc::clone(&self.state),
            trace: Rc::clone(&self.trace),
        }
    }

    #[must_use]
    pub fn action(&self, action: Action) -> ActionLocator {
        ActionLocator {
            state: Rc::clone(&self.state),
            trace: Rc::clone(&self.trace),
            action,
        }
    }

    pub fn expect_connection(&self, expected: ConnectionState) -> Result<()> {
        let state = self.state.borrow();
        if state.view.connection != expected {
            return Err(Error::Expectation {
                expectation: format!("connection is {expected:?}"),
                actual: format!("{:?}", state.view.connection),
                artifact: self.trace.borrow().persist(),
            });
        }
        Ok(())
    }

    #[must_use]
    pub fn rows(&self) -> Vec<String> {
        rows(&self.state.borrow().buffer)
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision
    }
}

pub struct ChatLocator {
    state: Rc<RefCell<RenderedState>>,
    trace: Rc<RefCell<Trace>>,
    title: String,
}

impl ChatLocator {
    pub fn expect_active(&self) -> Result<()> {
        let state = self.state.borrow();
        let matches = state
            .semantics
            .iter()
            .filter(|node| node.role == SemanticRole::Chat && node.name == self.title)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(Error::LocatorCardinality {
                query: format!("Chat named {}", self.title),
                matches: matches.len(),
                artifact: self.trace.borrow().persist(),
            });
        }
        if !matches[0].active {
            return Err(Error::Expectation {
                expectation: format!("Chat {} is Active Chat", self.title),
                actual: state
                    .semantics
                    .iter()
                    .find(|node| node.role == SemanticRole::Chat && node.active)
                    .map_or_else(|| "none".to_owned(), |node| node.name.clone()),
                artifact: self.trace.borrow().persist(),
            });
        }
        Ok(())
    }
}

enum MessageQuery {
    Id(MessageId),
    Text(String),
}
