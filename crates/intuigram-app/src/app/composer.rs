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
