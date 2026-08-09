use intuigram_app::{InlineImage, MediaCard, PollOptionView};

use super::media_image::{ImageRenderContext, render_image, render_loading_image};
use super::*;

#[derive(Clone, Copy)]
pub(super) enum AlbumPosition {
    None,
    First,
    Middle,
    Last,
    Only,
}

pub(super) struct MediaRenderContext {
    pub(super) active: bool,
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
    image_id: Option<rasterm::ImageId>,
    graphics: &mut GraphicsFrame,
) -> Vec<Line<'static>> {
    let MediaRenderContext {
        active,
        selected,
        forwarded,
        focused,
        album,
        animation_frame,
        max_height,
    } = context;
    let mut lines = Vec::new();
    if let Some(preview) = preview {
        lines.extend(render_image(
            preview,
            ImageRenderContext {
                id: image_id,
                active,
                selected,
                forwarded,
                focused,
                max_height,
            },
            graphics,
        ));
    } else if loading {
        lines.extend(render_loading_image(
            selected,
            active,
            forwarded,
            animation_frame,
            max_height,
        ));
    } else {
        let mut card = content_prefix(active, selected, forwarded);
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
    if preview.is_none() && !loading {
        lines.extend(media.details.iter().map(|detail| {
            let mut spans = content_prefix(active, selected, forwarded);
            spans.push(Span::styled(
                format!("  {detail}"),
                Style::default().fg(MUTED_TEXT),
            ));
            Line::from(spans)
        }));
    }
    if let Some(poll) = &media.poll {
        lines.extend(
            poll.options
                .iter()
                .map(|option| poll_option_line(option, active, selected, forwarded)),
        );
        if let Some(total) = poll.total_voters {
            let state = if poll.closed { " · closed" } else { "" };
            let choice = if poll.multiple_choice {
                " · multiple choice"
            } else {
                ""
            };
            let mut spans = content_prefix(active, selected, forwarded);
            spans.push(Span::styled(
                format!("  {total} voters{choice}{state}"),
                Style::default().fg(MUTED_TEXT),
            ));
            lines.push(Line::from(spans));
        }
        if let Some(solution) = &poll.solution {
            let mut spans = content_prefix(active, selected, forwarded);
            spans.push(Span::styled(
                format!("  Explanation: {solution}"),
                Style::default().fg(PRIMARY),
            ));
            lines.push(Line::from(spans));
        }
    }
    lines
}

fn poll_option_line(
    option: &PollOptionView,
    active: bool,
    selected: bool,
    forwarded: bool,
) -> Line<'static> {
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
    let mut spans = content_prefix(active, selected, forwarded);
    spans.push(Span::styled(
        format!("  {marker} {}{votes}", option.text),
        style,
    ));
    Line::from(spans)
}
