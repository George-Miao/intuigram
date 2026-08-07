pub struct ComposerLocator {
    pub(super) state: Rc<RefCell<RenderedState>>,
    pub(super) trace: Rc<RefCell<Trace>>,
}

/// Lazy query for one Media Card in the latest rendered semantic tree.
pub struct MediaCardLocator {
    pub(super) state: Rc<RefCell<RenderedState>>,
    pub(super) trace: Rc<RefCell<Trace>>,
    pub(super) title: String,
}

impl MediaCardLocator {
    /// Requires one matching card with the expected informative fallback.
    pub fn expect_description(&self, expected: &str) -> Result<()> {
        let state = self.state.borrow();
        let matches = state
            .semantics
            .iter()
            .filter(|node| node.role == SemanticRole::MediaCard && node.name == self.title)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(Error::LocatorCardinality {
                query: format!("Media Card named {}", self.title),
                matches: matches.len(),
                artifact: self.trace.borrow().persist(),
            });
        }
        let actual = matches[0].description.as_deref();
        if actual != Some(expected) {
            return Err(Error::Expectation {
                expectation: format!("Media Card {} has description {expected:?}", self.title),
                actual: format!("{actual:?}"),
                artifact: self.trace.borrow().persist(),
            });
        }
        Ok(())
    }
}

impl ComposerLocator {
    pub fn expect_focused(&self) -> Result<()> {
        let state = self.state.borrow();
        if !state
            .semantics
            .iter()
            .any(|node| node.role == SemanticRole::Composer && node.focused)
        {
            return Err(Error::Expectation {
                expectation: "Composer is focused".to_owned(),
                actual: format!("{:?}", state.view.focus),
                artifact: self.trace.borrow().persist(),
            });
        }
        Ok(())
    }

    pub fn expect_text(&self, expected: &str) -> Result<()> {
        let state = self.state.borrow();
        if state.view.composer.text != expected {
            return Err(Error::Expectation {
                expectation: format!("Composer text is {expected:?}"),
                actual: format!("{:?}", state.view.composer.text),
                artifact: self.trace.borrow().persist(),
            });
        }
        Ok(())
    }
}

pub struct ActionLocator {
    pub(super) state: Rc<RefCell<RenderedState>>,
    pub(super) trace: Rc<RefCell<Trace>>,
    pub(super) action: Action,
}

impl ActionLocator {
    pub fn expect_available(&self) -> Result<()> {
        if !self
            .state
            .borrow()
            .semantics
            .iter()
            .any(|node| node.role == SemanticRole::Action && node.action == Some(self.action))
        {
            return Err(Error::ActionUnavailable {
                action: self.action,
                artifact: self.trace.borrow().persist(),
            });
        }
        Ok(())
    }

    pub fn expect_unavailable(&self) -> Result<()> {
        if self
            .state
            .borrow()
            .semantics
            .iter()
            .any(|node| node.role == SemanticRole::Action && node.action == Some(self.action))
        {
            return Err(Error::Expectation {
                expectation: format!("action {:?} is unavailable", self.action),
                actual: "action is displayed".to_owned(),
                artifact: self.trace.borrow().persist(),
            });
        }
        Ok(())
    }
}

pub(crate) fn rows(buffer: &Buffer) -> Vec<String> {
    (0..buffer.area.height)
        .map(|y| {
            let mut row = String::new();
            for x in 0..buffer.area.width {
                row.push_str(buffer[(x, y)].symbol());
            }
            row.trim_end().to_owned()
        })
        .collect()
}
use super::*;
