impl App {
    pub(super) fn insert_composer_text(&mut self, text: &str) {
        let cursor = valid_cursor(&self.view.composer.text, self.view.composer.cursor);
        self.view.composer.text.insert_str(cursor, text);
        self.view.composer.cursor = cursor.saturating_add(text.len());
    }

    pub(super) fn backspace_composer(&mut self) {
        let cursor = valid_cursor(&self.view.composer.text, self.view.composer.cursor);
        let Some(previous) = previous_boundary(&self.view.composer.text, cursor) else {
            return;
        };
        self.view.composer.text.replace_range(previous..cursor, "");
        self.view.composer.cursor = previous;
    }

    pub(super) fn move_composer_cursor(&mut self, movement: ComposerMovement) {
        if self.view.focus != Focus::Composer {
            return;
        }
        let text = &self.view.composer.text;
        let cursor = valid_cursor(text, self.view.composer.cursor);
        self.view.composer.cursor = match movement {
            ComposerMovement::Left => previous_boundary(text, cursor).unwrap_or(cursor),
            ComposerMovement::Right => next_boundary(text, cursor).unwrap_or(cursor),
            ComposerMovement::Up => vertical_cursor(text, cursor, false),
            ComposerMovement::Down => vertical_cursor(text, cursor, true),
        };
    }

    pub(super) fn set_composer_cursor(&mut self, cursor: usize) {
        if self.active_chat_id().is_none() {
            return;
        }
        self.view.focus = Focus::Composer;
        self.view.active_message = None;
        self.view.composer.cursor = valid_cursor(&self.view.composer.text, cursor);
    }

    pub(super) fn move_active_attachment(&mut self, forward: bool) {
        let attachments = &mut self.view.composer.attachments;
        let Some(current) = active_attachment_index(attachments) else {
            return;
        };
        let next = if forward {
            current
                .saturating_add(1)
                .min(attachments.len().saturating_sub(1))
        } else {
            current.saturating_sub(1)
        };
        for (index, attachment) in attachments.iter_mut().enumerate() {
            attachment.active = index == next;
        }
    }

    pub(super) fn remove_active_attachment(&mut self) -> Option<Effect> {
        let attachments = &mut self.view.composer.attachments;
        let current = active_attachment_index(attachments)?;
        let removed = attachments.remove(current);
        let next = current.min(attachments.len().saturating_sub(1));
        for (index, attachment) in attachments.iter_mut().enumerate() {
            attachment.active = index == next;
        }
        Some(Effect::DiscardAttachment {
            attachment: removed.id,
        })
    }
}

pub(super) fn append_attachments(
    existing: &mut Vec<AttachmentView>,
    mut incoming: Vec<AttachmentView>,
    replace: bool,
) {
    if incoming.is_empty() {
        return;
    }
    if replace {
        existing.clear();
    } else {
        for attachment in existing.iter_mut() {
            attachment.active = false;
        }
    }
    for attachment in &mut incoming {
        attachment.active = false;
    }
    if let Some(last) = incoming.last_mut() {
        last.active = true;
    }
    existing.extend(incoming);
}

fn active_attachment_index(attachments: &[AttachmentView]) -> Option<usize> {
    attachments
        .iter()
        .position(|attachment| attachment.active)
        .or_else(|| attachments.len().checked_sub(1))
}

fn valid_cursor(text: &str, cursor: usize) -> usize {
    let mut cursor = cursor.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor = cursor.saturating_sub(1);
    }
    cursor
}

fn previous_boundary(text: &str, cursor: usize) -> Option<usize> {
    text[..cursor]
        .char_indices()
        .next_back()
        .map(|(index, _)| index)
}

fn next_boundary(text: &str, cursor: usize) -> Option<usize> {
    text[cursor..]
        .char_indices()
        .nth(1)
        .map(|(index, _)| cursor.saturating_add(index))
        .or((cursor < text.len()).then_some(text.len()))
}

fn vertical_cursor(text: &str, cursor: usize, down: bool) -> usize {
    let line_start = text[..cursor].rfind('\n').map_or(0, |index| index + 1);
    let column = text[line_start..cursor].chars().count();
    if down {
        let Some(next_start) = text[cursor..].find('\n').map(|index| cursor + index + 1) else {
            return cursor;
        };
        let next_end = text[next_start..]
            .find('\n')
            .map_or(text.len(), |index| next_start + index);
        byte_at_column(text, next_start, next_end, column)
    } else {
        let Some(previous_end) = line_start.checked_sub(1) else {
            return cursor;
        };
        let previous_start = text[..previous_end]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        byte_at_column(text, previous_start, previous_end, column)
    }
}

fn byte_at_column(text: &str, start: usize, end: usize, column: usize) -> usize {
    text[start..end]
        .char_indices()
        .nth(column)
        .map_or(end, |(offset, _)| start + offset)
}

use super::*;
