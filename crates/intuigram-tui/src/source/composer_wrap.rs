use super::*;

pub(super) struct WrappedText {
    pub(super) rows: Vec<String>,
    pub(super) cursor_row: usize,
    pub(super) cursor_column: usize,
}

pub(super) fn wrap_text(text: &str, cursor: usize, width: usize) -> WrappedText {
    let cursor = valid_cursor(text, cursor);
    let mut rows = vec![String::new()];
    let (mut row, mut column) = (0_usize, 0_usize);
    let mut cursor_position = None;
    for (index, character) in text.char_indices() {
        let character_width = Line::from(character.to_string()).width();
        if character != '\n' && column > 0 && column.saturating_add(character_width) > width {
            rows.push(String::new());
            row += 1;
            column = 0;
        }
        if index == cursor {
            cursor_position = Some((row, column));
        }
        if character == '\n' {
            rows.push(String::new());
            row += 1;
            column = 0;
        } else {
            rows[row].push(character);
            column = column.saturating_add(character_width);
        }
    }
    if cursor == text.len() && column >= width {
        rows.push(String::new());
        row += 1;
        column = 0;
    }
    let (cursor_row, cursor_column) = cursor_position.unwrap_or((row, column));
    WrappedText {
        rows,
        cursor_row,
        cursor_column,
    }
}

fn valid_cursor(text: &str, cursor: usize) -> usize {
    let mut cursor = cursor.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor = cursor.saturating_sub(1);
    }
    cursor
}
