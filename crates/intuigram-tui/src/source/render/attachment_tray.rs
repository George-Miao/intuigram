use intuigram_lib::{AttachmentId, AttachmentKind, AttachmentView};
use rasterm::{CellBounds, CellSize, Image, fit_cells, text_cells, unicode_placeholder};

use super::chrome::{interaction_rule, surface_style};
use super::*;
use crate::source::graphics::{attachment_image_id, image_color};

pub(super) const HEIGHT: u16 = PREVIEW_HEIGHT + 1;
const ITEM_WIDTH: u16 = 24;
const PREVIEW_WIDTH: u16 = 4;
const PREVIEW_HEIGHT: u16 = 2;

pub(super) fn render(frame: &mut Frame<'_>, area: Rect, view: &View, graphics: &mut GraphicsFrame) {
    let focused = focus_visible(view, Focus::Composer);
    frame.render_widget(Paragraph::new("").style(surface_style(focused)), area);
    if area.is_empty() {
        return;
    }

    let width = ITEM_WIDTH.min(area.width.max(1));
    let visible = usize::from((area.width.saturating_sub(1) / width).max(1));
    let active = view
        .composer
        .attachments
        .iter()
        .position(|attachment| attachment.active)
        .unwrap_or(0);
    let start = active / visible * visible;
    let attachments = view
        .composer
        .attachments
        .iter()
        .skip(start)
        .take(visible)
        .collect::<Vec<_>>();
    let mut rows = vec![Line::from("")];
    let mut previews = attachments
        .iter()
        .map(|attachment| preview_rows(attachment, graphics, focused))
        .collect::<Vec<_>>();
    for row in 0..usize::from(PREVIEW_HEIGHT) {
        let mut spans = vec![Span::raw(" ")];
        for (attachment, preview) in attachments.iter().zip(&mut previews) {
            spans.push(interaction_rule(focused && attachment.active));
            spans.append(&mut preview[row]);
            spans.push(Span::raw(" "));
            let label_width = usize::from(width.saturating_sub(PREVIEW_WIDTH + 3));
            let (label, style) = if row == 0 {
                (
                    fit_text(&attachment.name, label_width),
                    if attachment.active {
                        Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(TEXT)
                    },
                )
            } else {
                (
                    fit_text(kind_label(attachment.kind), label_width),
                    Style::default().fg(MUTED_TEXT),
                )
            };
            let padding = label_width.saturating_sub(Line::from(label.as_str()).width());
            spans.push(Span::styled(label, style));
            spans.push(Span::raw(" ".repeat(padding)));
        }
        rows.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(rows).style(surface_style(focused)), area);
}

fn preview_rows(
    attachment: &AttachmentView,
    graphics: &mut GraphicsFrame,
    focused: bool,
) -> [Vec<Span<'static>>; PREVIEW_HEIGHT as usize] {
    let Some(image) = attachment.preview.as_ref() else {
        return [
            vec![Span::styled(
                kind_fallback(attachment.kind),
                Style::default().fg(MUTED_TEXT),
            )],
            vec![Span::raw(" ".repeat(usize::from(PREVIEW_WIDTH)))],
        ];
    };
    let size = fit_cells(
        u32::from(image.width()),
        u32::from(image.height()),
        CellBounds {
            columns: PREVIEW_WIDTH,
            rows: PREVIEW_HEIGHT,
        },
    );
    if graphics.protocol().uses_placements() {
        native_preview(attachment.id, image, size, graphics, focused)
    } else {
        text_preview(image, size, focused)
    }
}

fn native_preview(
    attachment: AttachmentId,
    image: &intuigram_lib::InlineImage,
    size: CellSize,
    graphics: &mut GraphicsFrame,
    focused: bool,
) -> [Vec<Span<'static>>; PREVIEW_HEIGHT as usize] {
    let id = attachment_image_id(attachment);
    graphics.push(id, image, size);
    let style = surface_style(focused).fg(image_color(id));
    std::array::from_fn(|row| {
        let mut spans = Vec::with_capacity(usize::from(PREVIEW_WIDTH));
        let rendered_columns = if row < usize::from(size.rows) {
            spans.extend((0..size.columns).map(|column| {
                let symbol = if graphics.protocol().uses_unicode_placeholders() {
                    unicode_placeholder(
                        u16::try_from(row).expect("the preview row fits in u16"),
                        column,
                    )
                    .expect("attachment previews stay within Kitty placeholder bounds")
                } else {
                    " ".to_owned()
                };
                Span::styled(symbol, style)
            }));
            size.columns
        } else {
            0
        };
        spans.push(Span::raw(" ".repeat(usize::from(
            PREVIEW_WIDTH.saturating_sub(rendered_columns),
        ))));
        spans
    })
}

fn text_preview(
    image: &intuigram_lib::InlineImage,
    size: CellSize,
    focused: bool,
) -> [Vec<Span<'static>>; PREVIEW_HEIGHT as usize] {
    let image = Image::from_rgba(
        u32::from(image.width()),
        u32::from(image.height()),
        image.rgba().to_vec(),
    )
    .expect("an Intuigram InlineImage has validated RGBA dimensions");
    let cells = text_cells(&image, size, surface_rgb(focused));
    std::array::from_fn(|row| {
        let mut spans = Vec::with_capacity(usize::from(PREVIEW_WIDTH));
        let rendered_columns = if row < usize::from(size.rows) {
            let offset = row.saturating_mul(usize::from(size.columns));
            spans.extend(
                cells[offset..offset + usize::from(size.columns)]
                    .iter()
                    .map(|cell| {
                        Span::styled(
                            "▀",
                            Style::default()
                                .fg(Color::Rgb(cell.upper.0, cell.upper.1, cell.upper.2))
                                .bg(Color::Rgb(cell.lower.0, cell.lower.1, cell.lower.2)),
                        )
                    }),
            );
            size.columns
        } else {
            0
        };
        spans.push(Span::raw(" ".repeat(usize::from(
            PREVIEW_WIDTH.saturating_sub(rendered_columns),
        ))));
        spans
    })
}

fn fit_text(text: &str, width: usize) -> String {
    let mut result = String::new();
    let mut used = 0_usize;
    for character in text.chars() {
        let character_width = Line::from(character.to_string()).width();
        if used.saturating_add(character_width) > width {
            break;
        }
        result.push(character);
        used = used.saturating_add(character_width);
    }
    result
}

const fn kind_label(kind: AttachmentKind) -> &'static str {
    match kind {
        AttachmentKind::Photo => "Photo",
        AttachmentKind::Video => "Video",
        AttachmentKind::File => "File",
    }
}

const fn kind_fallback(kind: AttachmentKind) -> &'static str {
    match kind {
        AttachmentKind::Photo => "IMG ",
        AttachmentKind::Video => "VID ",
        AttachmentKind::File => "FILE",
    }
}

const fn surface_rgb(focused: bool) -> (u8, u8, u8) {
    if focused {
        (230, 226, 204)
    } else {
        (244, 240, 217)
    }
}
