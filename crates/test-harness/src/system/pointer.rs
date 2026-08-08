use crossterm::event::{Event, KeyModifiers, MouseEvent, MouseEventKind};
use intuigram_app::{ScrollDirection, ScrollTarget};
use intuigram_tui::{SemanticRole, render_test_frame};

use super::TestSystem;
use crate::error::{Error, Result};

impl TestSystem {
    pub fn scroll(&mut self, target: ScrollTarget, direction: ScrollDirection) -> Result<()> {
        let frame = render_test_frame(self.application.view(), self.terminal.0, self.terminal.1);
        let role = match target {
            ScrollTarget::Chats => SemanticRole::ChatList,
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
}
