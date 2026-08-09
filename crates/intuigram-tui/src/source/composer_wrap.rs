use super::*;

pub(super) struct WrappedText {
    pub(super) rows: Vec<String>,
    row_starts: Vec<usize>,
    pub(super) cursor_row: usize,
    pub(super) cursor_column: usize,
}

pub(super) fn wrap_text(text: &str, cursor: usize, width: usize) -> WrappedText {
    let cursor = valid_cursor(text, cursor);
    let mut rows = vec![String::new()];
    let mut row_starts = vec![0];
    let (mut row, mut column) = (0_usize, 0_usize);
    let mut cursor_position = None;
    for (index, character) in text.char_indices() {
        let character_width = Line::from(character.to_string()).width();
        if character != '\n' && column > 0 && column.saturating_add(character_width) > width {
            rows.push(String::new());
            row_starts.push(index);
            row += 1;
            column = 0;
        }
        if index == cursor {
            cursor_position = Some((row, column));
        }
        if character == '\n' {
            rows.push(String::new());
            row_starts.push(index.saturating_add(character.len_utf8()));
            row += 1;
            column = 0;
        } else {
            rows[row].push(character);
            column = column.saturating_add(character_width);
        }
    }
    if cursor == text.len() && column >= width {
        rows.push(String::new());
        row_starts.push(text.len());
        row += 1;
        column = 0;
    }
    let (cursor_row, cursor_column) = cursor_position.unwrap_or((row, column));
    WrappedText {
        rows,
        row_starts,
        cursor_row,
        cursor_column,
    }
}

impl WrappedText {
    pub(super) fn cursor_at(&self, text: &str, row: usize, column: usize) -> Option<usize> {
        let start = *self.row_starts.get(row)?;
        let mut end = self
            .row_starts
            .get(row.saturating_add(1))
            .copied()
            .unwrap_or(text.len());
        if end > start && text.as_bytes().get(end - 1) == Some(&b'\n') {
            end -= 1;
        }
        let mut cells = 0;
        for (offset, character) in text[start..end].char_indices() {
            let width = Line::from(character.to_string()).width();
            if cells >= column || cells.saturating_add(width) > column {
                return Some(start.saturating_add(offset));
            }
            cells = cells.saturating_add(width);
        }
        Some(end)
    }
}

fn valid_cursor(text: &str, cursor: usize) -> usize {
    let mut cursor = cursor.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor = cursor.saturating_sub(1);
    }
    cursor
}
