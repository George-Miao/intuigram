use intuigram_app::InlineImage;
use rasterm::{CellBounds, CellSize, Image, fit_cells, text_cells, unicode_placeholder};

use super::*;

const WIDTH: u16 = 32;
const HEIGHT: u16 = 6;
pub(super) struct ImageRenderContext {
    pub(super) id: Option<u32>,
    pub(super) active: bool,
    pub(super) selected: bool,
    pub(super) forwarded: bool,
    pub(super) focused: bool,
    pub(super) max_height: u16,
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
            columns: WIDTH,
            rows: HEIGHT.min(context.max_height),
        },
    );
    if graphics.protocol().is_native()
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
    max_height: u16,
) -> Vec<Line<'static>> {
    let highlight = u16::from(animation_frame) % WIDTH;
    let height = HEIGHT.min(max_height);
    (0..height)
        .map(|row| {
            let mut spans = Vec::with_capacity(usize::from(WIDTH).saturating_add(1));
            spans.extend(content_prefix(active, selected, forwarded));
            spans.extend((0..WIDTH).map(|column| {
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

fn render_native_image(
    image: &InlineImage,
    id: u32,
    size: CellSize,
    context: ImageRenderContext,
    graphics: &mut GraphicsFrame,
) -> Vec<Line<'static>> {
    graphics.push(id, image, size);
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
