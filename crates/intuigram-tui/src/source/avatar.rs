use rasterm::{CellSize, Image, ImageId, text_cells, unicode_placeholder};

use super::*;

const AVATAR_COLUMNS: u16 = 2;
const AVATAR_ROWS: u16 = 1;

pub(super) fn avatar_badge(name: &str) -> Span<'static> {
    Span::styled(
        format!("[{}] ", avatar_initials(name)),
        Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
    )
}

pub(super) fn avatar_spans(
    view: &View,
    peer: Option<ChatId>,
    name: &str,
    id: Option<ImageId>,
    graphics: &mut GraphicsFrame,
    focused: bool,
) -> Vec<Span<'static>> {
    let Some((image, id)) = peer
        .and_then(|peer| {
            view.avatars
                .iter()
                .find(|avatar| avatar.avatar.peer == peer)
        })
        .zip(id)
    else {
        return vec![avatar_badge(name)];
    };
    let size = CellSize {
        columns: AVATAR_COLUMNS,
        rows: AVATAR_ROWS,
    };
    let mut spans = if graphics.protocol().uses_placements() {
        graphics.push(id, &image.image, size);
        let foreground = graphics::image_color(id);
        (0..AVATAR_COLUMNS)
            .map(|column| {
                let symbol = if graphics.protocol().uses_unicode_placeholders() {
                    unicode_placeholder(0, column)
                        .expect("avatar placeholders remain inside Kitty's coordinate limit")
                } else {
                    " ".to_owned()
                };
                let style = Style::default().fg(foreground);
                Span::styled(
                    symbol,
                    if focused {
                        style.bg(FOCUSED_SURFACE_BACKGROUND)
                    } else {
                        style
                    },
                )
            })
            .collect::<Vec<_>>()
    } else {
        let image = Image::from_rgba(
            u32::from(image.image.width()),
            u32::from(image.image.height()),
            image.image.rgba().to_vec(),
        )
        .expect("an Intuigram InlineImage has already validated its RGBA dimensions");
        let background = if focused {
            (230, 226, 204)
        } else {
            (244, 240, 217)
        };
        text_cells(&image, size, background)
            .into_iter()
            .map(|cell| {
                Span::styled(
                    "▀",
                    Style::default()
                        .fg(Color::Rgb(cell.upper.0, cell.upper.1, cell.upper.2))
                        .bg(Color::Rgb(cell.lower.0, cell.lower.1, cell.lower.2)),
                )
            })
            .collect()
    };
    spans.push(Span::raw(" "));
    spans
}

pub(super) fn avatar_width(view: &View, peer: Option<ChatId>, name: &str) -> usize {
    if peer.is_some_and(|peer| view.avatars.iter().any(|avatar| avatar.avatar.peer == peer)) {
        usize::from(AVATAR_COLUMNS.saturating_add(1))
    } else {
        Line::from(avatar_badge(name)).width()
    }
}

fn avatar_initials(name: &str) -> String {
    let words = name
        .split_whitespace()
        .filter_map(|word| word.chars().find(|character| character.is_alphanumeric()))
        .collect::<Vec<_>>();
    let initials = match words.as_slice() {
        [] => vec!['?'],
        [_] => name
            .chars()
            .filter(|character| character.is_alphanumeric())
            .take(2)
            .collect(),
        [first, rest @ ..] => vec![
            *first,
            *rest.last().expect("multiple words have a last item"),
        ],
    };
    initials.into_iter().flat_map(char::to_uppercase).collect()
}

#[cfg(test)]
mod tests {
    use super::avatar_initials;

    #[test]
    fn initials_are_deterministic_for_words_unicode_and_empty_names() {
        assert_eq!(avatar_initials("Intuigram Team"), "IT");
        assert_eq!(avatar_initials("alice"), "AL");
        assert_eq!(avatar_initials("李 雷"), "李雷");
        assert_eq!(avatar_initials(""), "?");
    }
}
