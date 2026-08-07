pub struct MessageLocator {
    pub(super) state: Rc<RefCell<RenderedState>>,
    pub(super) trace: Rc<RefCell<Trace>>,
    pub(super) query: MessageQuery,
}

impl MessageLocator {
    pub fn expect_reaction(&self, label: &str, count: u32, chosen: bool) -> Result<()> {
        let state = self.state.borrow();
        let message = state
            .view
            .messages
            .iter()
            .find(|message| match &self.query {
                MessageQuery::Id(id) => message.id == *id,
                MessageQuery::Text(text) => message.body == *text,
            });
        let actual = message.and_then(|message| {
            message
                .details
                .reactions
                .iter()
                .find(|reaction| reaction.label == label)
        });
        if actual.is_some_and(|reaction| reaction.count == count && reaction.chosen == chosen) {
            Ok(())
        } else {
            Err(Error::Expectation {
                expectation: format!(
                    "{} has reaction {label:?} count {count} chosen {chosen}",
                    self.describe()
                ),
                actual: format!("{actual:?}"),
                artifact: self.trace.borrow().persist(),
            })
        }
    }

    pub fn expect_absent(&self) -> Result<()> {
        let state = self.state.borrow();
        let matches = state
            .semantics
            .iter()
            .filter(|node| node.role == SemanticRole::Message)
            .filter(|node| match &self.query {
                MessageQuery::Id(id) => node.domain_id == Some(id.0),
                MessageQuery::Text(text) => node.name == *text,
            })
            .count();
        if matches == 0 {
            Ok(())
        } else {
            Err(Error::Expectation {
                expectation: format!("{} is absent", self.describe()),
                actual: format!("{matches} matching Message(s)"),
                artifact: self.trace.borrow().persist(),
            })
        }
    }

    pub fn expect_sender(&self, expected: &str) -> Result<()> {
        let state = self.state.borrow();
        let matches = state
            .semantics
            .iter()
            .filter(|node| node.role == SemanticRole::Message)
            .filter(|node| match &self.query {
                MessageQuery::Id(id) => node.domain_id == Some(id.0),
                MessageQuery::Text(text) => node.name == *text,
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(Error::LocatorCardinality {
                query: self.describe(),
                matches: matches.len(),
                artifact: self.trace.borrow().persist(),
            });
        }
        let actual = matches[0].description.as_deref();
        if actual == Some(expected) {
            Ok(())
        } else {
            Err(Error::Expectation {
                expectation: format!("{} has sender {expected:?}", self.describe()),
                actual: format!("{actual:?}"),
                artifact: self.trace.borrow().persist(),
            })
        }
    }

    pub fn expect_delivery(&self, expected: DeliveryState) -> Result<()> {
        let state = self.state.borrow();
        let matches = state
            .semantics
            .iter()
            .filter(|node| node.role == SemanticRole::Message)
            .filter(|node| match &self.query {
                MessageQuery::Id(id) => node.domain_id == Some(id.0),
                MessageQuery::Text(text) => node.name == *text,
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(Error::LocatorCardinality {
                query: self.describe(),
                matches: matches.len(),
                artifact: self.trace.borrow().persist(),
            });
        }
        let actual = matches[0].delivery;
        if actual != Some(expected) {
            return Err(Error::Expectation {
                expectation: format!("{} has delivery {expected:?}", self.describe()),
                actual: format!("{actual:?}"),
                artifact: self.trace.borrow().persist(),
            });
        }
        Ok(())
    }

    pub fn expect_active(&self) -> Result<()> {
        let state = self.state.borrow();
        let matches = state
            .semantics
            .iter()
            .filter(|node| node.role == SemanticRole::Message)
            .filter(|node| match &self.query {
                MessageQuery::Id(id) => node.domain_id == Some(id.0),
                MessageQuery::Text(text) => node.name == *text,
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(Error::LocatorCardinality {
                query: self.describe(),
                matches: matches.len(),
                artifact: self.trace.borrow().persist(),
            });
        }
        if !matches[0].active {
            return Err(Error::Expectation {
                expectation: format!("{} is Active Message", self.describe()),
                actual: state
                    .semantics
                    .iter()
                    .find(|node| node.role == SemanticRole::Message && node.active)
                    .map_or_else(|| "none".to_owned(), |node| node.name.clone()),
                artifact: self.trace.borrow().persist(),
            });
        }
        Ok(())
    }

    fn describe(&self) -> String {
        match &self.query {
            MessageQuery::Id(id) => format!("Message {}", id.0),
            MessageQuery::Text(text) => format!("Message with text {text:?}"),
        }
    }
}
use super::*;
