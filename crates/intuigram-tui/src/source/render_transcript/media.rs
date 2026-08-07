use intuigram_app::{InlineImage, MediaCard, PollOptionView};

use super::*;

pub(super) enum AlbumPosition {
    None,
    First,
    Middle,
    Last,
    Only,
}

impl AlbumPosition {
    pub(super) const fn from_neighbors(before: bool, after: bool) -> Self {
        match (before, after) {
            (false, false) => Self::Only,
            (false, true) => Self::First,
            (true, true) => Self::Middle,
            (true, false) => Self::Last,
        }
    }

    const fn label(&self) -> &'static str {
        match self {
            Self::None => "",
            Self::First => "Album 1 · ",
            Self::Middle => "Album · ",
            Self::Last => "Album end · ",
            Self::Only => "Album · ",
        }
    }
}

pub(super) fn media_line_count(media: &MediaCard, preview: Option<&InlineImage>) -> u16 {
    let poll_lines = media.poll.as_ref().map_or(0, |poll| {
        poll.options.len()
            + usize::from(poll.total_voters.is_some())
            + usize::from(poll.solution.is_some())
    });
    let preview_lines = preview.map_or(0, |image| image.height().div_ceil(2));
    u16::try_from(1 + media.details.len() + poll_lines)
        .unwrap_or(u16::MAX)
        .saturating_add(preview_lines)
}

pub(super) fn render_media(
    media: &MediaCard,
    preview: Option<&InlineImage>,
    selected: bool,
    focused: bool,
    album: AlbumPosition,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        selection_rule(selected),
        Span::styled(
            format!("[{}{}]", album.label(), media.title),
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}", media.description),
            Style::default().fg(MUTED_TEXT),
        ),
    ])];
    if let Some(preview) = preview {
        lines.extend(render_inline_image(preview, selected, focused));
    }
    lines.extend(media.details.iter().map(|detail| {
        Line::from(vec![
            selection_rule(selected),
            Span::styled(format!("  {detail}"), Style::default().fg(MUTED_TEXT)),
        ])
    }));
    if let Some(poll) = &media.poll {
        lines.extend(
            poll.options
                .iter()
                .map(|option| poll_option_line(option, selected)),
        );
        if let Some(total) = poll.total_voters {
            let state = if poll.closed { " · closed" } else { "" };
            let choice = if poll.multiple_choice {
                " · multiple choice"
            } else {
                ""
            };
            lines.push(Line::from(vec![
                selection_rule(selected),
                Span::styled(
                    format!("  {total} voters{choice}{state}"),
                    Style::default().fg(MUTED_TEXT),
                ),
            ]));
        }
        if let Some(solution) = &poll.solution {
            lines.push(Line::from(vec![
                selection_rule(selected),
                Span::styled(
                    format!("  Explanation: {solution}"),
                    Style::default().fg(PRIMARY),
                ),
            ]));
        }
    }
    lines
}

fn render_inline_image(image: &InlineImage, selected: bool, focused: bool) -> Vec<Line<'static>> {
    let background = if focused {
        (230, 226, 204)
    } else {
        (244, 240, 217)
    };
    (0..image.height())
        .step_by(2)
        .map(|upper_y| {
            let mut spans = Vec::with_capacity(usize::from(image.width()).saturating_add(1));
            spans.push(selection_rule(selected));
            spans.extend((0..image.width()).map(|x| {
                let upper = pixel(image, x, upper_y, background);
                let lower = if upper_y + 1 < image.height() {
                    pixel(image, x, upper_y + 1, background)
                } else {
                    background
                };
                Span::styled(
                    "▀",
                    Style::default()
                        .fg(Color::Rgb(upper.0, upper.1, upper.2))
                        .bg(Color::Rgb(lower.0, lower.1, lower.2)),
                )
            }));
            Line::from(spans)
        })
        .collect()
}

fn pixel(image: &InlineImage, x: u16, y: u16, background: (u8, u8, u8)) -> (u8, u8, u8) {
    let offset = (usize::from(y) * usize::from(image.width()) + usize::from(x)) * 4;
    let rgba = &image.rgba()[offset..offset + 4];
    (
        blend(rgba[0], background.0, rgba[3]),
        blend(rgba[1], background.1, rgba[3]),
        blend(rgba[2], background.2, rgba[3]),
    )
}

fn blend(channel: u8, background: u8, alpha: u8) -> u8 {
    let alpha = u16::from(alpha);
    let blended =
        u16::from(channel) * alpha + u16::from(background) * (u16::from(u8::MAX) - alpha) + 127;
    u8::try_from(blended / u16::from(u8::MAX))
        .expect("alpha blending two u8 channels always produces one u8 channel")
}

fn poll_option_line(option: &PollOptionView, selected: bool) -> Line<'static> {
    let marker = if option.correct {
        "✓"
    } else if option.chosen {
        "●"
    } else {
        "○"
    };
    let votes = option
        .voters
        .map_or_else(String::new, |votes| format!(" · {votes}"));
    let style = if option.correct || option.chosen {
        Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    Line::from(vec![
        selection_rule(selected),
        Span::styled(format!("  {marker} {}{votes}", option.text), style),
    ])
}
