use intuigram_app::InlineImage;
use rasterm::{CellBounds, CellSize, Image, ImageId, fit_cells, text_cells, unicode_placeholder};

use super::*;

const WIDTH: u16 = 48;
const HEIGHT: u16 = 12;
pub(super) struct ImageRenderContext {
    pub(super) id: Option<ImageId>,
    pub(super) active: bool,
    pub(super) selected: bool,
    pub(super) forwarded: bool,
    pub(super) focused: bool,
    pub(super) max_width: u16,
    pub(super) max_height: u16,
    pub(super) content_indent: usize,
}

pub(super) fn render_image(
    image: &InlineImage,
    context: ImageRenderContext,
    graphics: &mut GraphicsFrame,
) -> Vec<Line<'static>> {
    let size = fit_cells(
        u32::from(image.width()),
        u32::from(image.height()),
        CellBounds {
            columns: WIDTH.min(context.max_width),
            rows: HEIGHT.min(context.max_height),
        },
    );
    if graphics.protocol().uses_placements()
        && let Some(id) = context.id
    {
        render_native_image(image, id, size, context, graphics)
    } else {
        render_text_image(image, size, context)
    }
}

pub(super) fn render_loading_image(
    selected: bool,
    active: bool,
    forwarded: bool,
    animation_frame: u8,
    max_width: u16,
    max_height: u16,
    content_indent: usize,
) -> Vec<Line<'static>> {
    let width = WIDTH.min(max_width);
    let highlight = u16::from(animation_frame) % width;
    let height = HEIGHT.min(max_height);
    let label = " loading image ";
    let label_width = u16::try_from(label.len()).expect("the loading label width fits in u16");
    let visible_label_width = label_width.min(width);
    let visible_label = &label[..usize::from(visible_label_width)];
    let label_start = width.saturating_sub(visible_label_width) / 2;
    let label_end = label_start.saturating_add(visible_label_width);
    (0..height)
        .map(|row| {
            let mut spans = Vec::with_capacity(usize::from(width).saturating_add(1));
            spans.extend(content_prefix(active, selected, forwarded, content_indent));
            spans.extend((0..label_start).map(|column| {
                let highlighted = column == highlight;
                Span::styled(
                    if highlighted { "▒" } else { "░" },
                    Style::default().fg(if highlighted { PRIMARY } else { MUTED_TEXT }),
                )
            }));
            if row == height / 2 {
                spans.push(Span::styled(
                    visible_label.to_owned(),
                    Style::default().fg(MUTED_TEXT),
                ));
            } else {
                spans.push(Span::styled(
                    "░".repeat(usize::from(label_end.saturating_sub(label_start))),
                    Style::default().fg(MUTED_TEXT),
                ));
            }
            spans.extend((label_end..width).map(|column| {
                let highlighted = column == highlight;
                Span::styled(
                    if highlighted { "▒" } else { "░" },
                    Style::default().fg(if highlighted { PRIMARY } else { MUTED_TEXT }),
                )
            }));
            Line::from(spans)
        })
        .collect()
}

fn render_native_image(
    image: &InlineImage,
    id: ImageId,
    size: CellSize,
    context: ImageRenderContext,
    graphics: &mut GraphicsFrame,
) -> Vec<Line<'static>> {
    graphics.push(id, image, size);
    let id = id.get();
    let foreground = Color::Rgb(
        u8::try_from((id >> 16) & 0xff).expect("a masked image ID byte fits in u8"),
        u8::try_from((id >> 8) & 0xff).expect("a masked image ID byte fits in u8"),
        u8::try_from(id & 0xff).expect("a masked image ID byte fits in u8"),
    );
    (0..size.rows)
        .map(|row| {
            let mut spans = Vec::with_capacity(usize::from(size.columns).saturating_add(1));
            spans.extend(content_prefix(
                context.active,
                context.selected,
                context.forwarded,
                context.content_indent,
            ));
            spans.extend((0..size.columns).map(|column| {
                let symbol = if graphics.protocol().uses_unicode_placeholders() {
                    unicode_placeholder(row, column)
                        .expect("the renderer caps Kitty placeholders to 32 rows and columns")
                } else {
                    " ".to_owned()
                };
                Span::styled(
                    symbol,
                    Style::default()
                        .fg(foreground)
                        .bg(image_background(context.focused)),
                )
            }));
            Line::from(spans)
        })
        .collect()
}

fn render_text_image(
    image: &InlineImage,
    size: CellSize,
    context: ImageRenderContext,
) -> Vec<Line<'static>> {
    let background = image_background_rgb(context.focused);
    let image = Image::from_rgba(
        u32::from(image.width()),
        u32::from(image.height()),
        image.rgba().to_vec(),
    )
    .expect("an Intuigram InlineImage has already validated its RGBA dimensions");
    let cells = text_cells(&image, size, background);
    (0..size.rows)
        .map(|line| {
            let mut spans = Vec::with_capacity(usize::from(size.columns).saturating_add(1));
            spans.extend(content_prefix(
                context.active,
                context.selected,
                context.forwarded,
                context.content_indent,
            ));
            let offset = usize::from(line) * usize::from(size.columns);
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
            Line::from(spans)
        })
        .collect()
}

fn image_background(focused: bool) -> Color {
    let (red, green, blue) = image_background_rgb(focused);
    Color::Rgb(red, green, blue)
}

const fn image_background_rgb(focused: bool) -> (u8, u8, u8) {
    if focused {
        (230, 226, 204)
    } else {
        (244, 240, 217)
    }
}
