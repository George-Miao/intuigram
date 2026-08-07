use intuigram_app::{InlineImage, MediaCard, PollOptionView};

use super::*;

const INLINE_IMAGE_WIDTH: u16 = 32;
const INLINE_IMAGE_HEIGHT: u16 = 6;

#[derive(Clone, Copy)]
pub(super) enum AlbumPosition {
    None,
    First,
    Middle,
    Last,
    Only,
}

pub(super) struct MediaRenderContext {
    pub(super) selected: bool,
    pub(super) forwarded: bool,
    pub(super) focused: bool,
    pub(super) album: AlbumPosition,
    pub(super) animation_frame: u8,
    pub(super) max_height: u16,
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

pub(super) fn render_media(
    media: &MediaCard,
    preview: Option<&InlineImage>,
    loading: bool,
    context: MediaRenderContext,
) -> Vec<Line<'static>> {
    let MediaRenderContext {
        selected,
        forwarded,
        focused,
        album,
        animation_frame,
        max_height,
    } = context;
    let mut lines = Vec::new();
    if let Some(preview) = preview {
        lines.extend(render_inline_image(
            preview, selected, forwarded, focused, max_height,
        ));
    } else if loading {
        lines.extend(render_image_placeholder(
            selected,
            forwarded,
            animation_frame,
            max_height,
        ));
    } else {
        let mut card = content_prefix(selected, forwarded);
        card.extend([
            Span::styled(
                format!("[{}{}]", album.label(), media.title),
                Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}", media.description),
                Style::default().fg(MUTED_TEXT),
            ),
        ]);
        lines.push(Line::from(card));
    }
    lines.extend(media.details.iter().map(|detail| {
        let mut spans = content_prefix(selected, forwarded);
        spans.push(Span::styled(
            format!("  {detail}"),
            Style::default().fg(MUTED_TEXT),
        ));
        Line::from(spans)
    }));
    if let Some(poll) = &media.poll {
        lines.extend(
            poll.options
                .iter()
                .map(|option| poll_option_line(option, selected, forwarded)),
        );
        if let Some(total) = poll.total_voters {
            let state = if poll.closed { " · closed" } else { "" };
            let choice = if poll.multiple_choice {
                " · multiple choice"
            } else {
                ""
            };
            let mut spans = content_prefix(selected, forwarded);
            spans.push(Span::styled(
                format!("  {total} voters{choice}{state}"),
                Style::default().fg(MUTED_TEXT),
            ));
            lines.push(Line::from(spans));
        }
        if let Some(solution) = &poll.solution {
            let mut spans = content_prefix(selected, forwarded);
            spans.push(Span::styled(
                format!("  Explanation: {solution}"),
                Style::default().fg(PRIMARY),
            ));
            lines.push(Line::from(spans));
        }
    }
    lines
}

fn render_inline_image(
    image: &InlineImage,
    selected: bool,
    forwarded: bool,
    focused: bool,
    max_height: u16,
) -> Vec<Line<'static>> {
    let background = if focused {
        (230, 226, 204)
    } else {
        (244, 240, 217)
    };
    (0..INLINE_IMAGE_HEIGHT.min(max_height))
        .map(|line| {
            let upper_y = line.saturating_mul(2);
            let mut spans = Vec::with_capacity(usize::from(INLINE_IMAGE_WIDTH).saturating_add(1));
            spans.extend(content_prefix(selected, forwarded));
            spans.extend((0..INLINE_IMAGE_WIDTH).map(|x| {
                if x >= image.width() || upper_y >= image.height() {
                    return Span::styled(
                        " ",
                        Style::default().bg(Color::Rgb(background.0, background.1, background.2)),
                    );
                }
                let upper = pixel(image, x, upper_y, background);
                let lower = if upper_y.saturating_add(1) < image.height() {
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

fn render_image_placeholder(
    selected: bool,
    forwarded: bool,
    animation_frame: u8,
    max_height: u16,
) -> Vec<Line<'static>> {
    let highlight = u16::from(animation_frame) % INLINE_IMAGE_WIDTH;
    let height = INLINE_IMAGE_HEIGHT.min(max_height);
    (0..height)
        .map(|row| {
            let mut spans = Vec::with_capacity(usize::from(INLINE_IMAGE_WIDTH).saturating_add(1));
            spans.extend(content_prefix(selected, forwarded));
            spans.extend((0..INLINE_IMAGE_WIDTH).map(|column| {
                let highlighted = column == highlight;
                Span::styled(
                    if highlighted { "▒" } else { "░" },
                    Style::default().fg(if highlighted { PRIMARY } else { MUTED_TEXT }),
                )
            }));
            if row == height / 2 {
                spans.push(Span::styled(
                    "  loading image",
                    Style::default().fg(MUTED_TEXT),
                ));
            }
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

fn poll_option_line(option: &PollOptionView, selected: bool, forwarded: bool) -> Line<'static> {
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
    let mut spans = content_prefix(selected, forwarded);
    spans.push(Span::styled(
        format!("  {marker} {}{votes}", option.text),
        style,
    ));
    Line::from(spans)
}
