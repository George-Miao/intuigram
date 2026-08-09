use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use intuigram_app::{ScrollDirection, ScrollTarget};
use intuigram_tui::{SemanticRole, render_test_frame};

use super::TestSystem;
use crate::error::{Error, Result};

impl TestSystem {
    pub fn click_composer(&mut self, column: u16, row: u16) -> Result<()> {
        self.click_semantic(SemanticRole::Composer, None, column, row)
    }

    pub fn click_action(&mut self, label: &str) -> Result<()> {
        self.click_semantic(SemanticRole::Action, Some(label), 0, 0)
    }

    pub fn scroll(&mut self, target: ScrollTarget, direction: ScrollDirection) -> Result<()> {
        let frame = render_test_frame(self.application.view(), self.terminal.0, self.terminal.1);
        let role = match target {
            ScrollTarget::Chats => SemanticRole::ChatList,
            ScrollTarget::Topics => SemanticRole::TopicList,
            ScrollTarget::Transcript => SemanticRole::Transcript,
        };
        let Some(bounds) = frame
            .semantics
            .iter()
            .find(|node| node.role == role)
            .map(|node| node.bounds)
        else {
            return Err(Error::UnavailableInput {
                event: format!("scroll {target:?} {direction:?}"),
                artifact: self.trace.borrow().persist(),
            });
        };
        self.deliver_event(Event::Mouse(MouseEvent {
            kind: match direction {
                ScrollDirection::Up => MouseEventKind::ScrollUp,
                ScrollDirection::Down => MouseEventKind::ScrollDown,
            },
            column: bounds.x,
            row: bounds.y,
            modifiers: KeyModifiers::NONE,
        }))
    }

    fn click_semantic(
        &mut self,
        role: SemanticRole,
        name: Option<&str>,
        column: u16,
        row: u16,
    ) -> Result<()> {
        let frame = render_test_frame(self.application.view(), self.terminal.0, self.terminal.1);
        let Some(node) = frame
            .semantics
            .iter()
            .find(|node| node.role == role && name.is_none_or(|name| node.name == name))
        else {
            return Err(Error::UnavailableInput {
                event: format!("click {role:?} {name:?}"),
                artifact: self.trace.borrow().persist(),
            });
        };
        self.deliver_event(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: node.bounds.x.saturating_add(column),
            row: node.bounds.y.saturating_add(row),
            modifiers: KeyModifiers::NONE,
        }))
    }
}
