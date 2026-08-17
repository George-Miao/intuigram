use intuigram_lib::InlineImage;
use rasterm::{CellBounds, CellSize, Image, ImageId, fit_cells, text_cells, unicode_placeholder};

use super::*;

const WIDTH: u16 = 48;
const HEIGHT: u16 = 12;
pub(super) struct ImageRenderContext {
    pub(super) id: Option<ImageId>,
    pub(super) active: bool,
    pub(super) selected: bool,
    pub(super) focused: bool,
    pub(super) max_width: u16,
    pub(super) max_height: u16,
    pub(super) component: MessageComponent,
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
    component: MessageComponent,
    animation_frame: u8,
    max_width: u16,
    max_height: u16,
) -> Vec<Line<'static>> {
    let width = WIDTH.min(max_width);
    let height = HEIGHT.min(max_height);
    let cycle = width.saturating_add(height).saturating_add(4);
    let phase = u16::from(animation_frame).wrapping_mul(2) % cycle;
    (0..height)
        .map(|row| {
            let mut spans = Vec::with_capacity(usize::from(width).saturating_add(1));
            spans.extend(component.prefix(active, selected));
            spans.extend((0..width).map(|column| {
                let distance = phase.abs_diff(column.saturating_add(row));
                let (symbol, color) = match distance {
                    0 => ("▓", PRIMARY),
                    1 => ("▒", PRIMARY),
                    2..=3 => ("▒", MUTED_TEXT),
                    _ => ("░", MUTED_TEXT),
                };
                Span::styled(symbol, Style::default().fg(color))
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
    prefixed_rows(
        native_image_rows(
            graphics.protocol(),
            id,
            size,
            image_background(context.focused),
        ),
        context,
    )
}

fn render_text_image(
    image: &InlineImage,
    size: CellSize,
    context: ImageRenderContext,
) -> Vec<Line<'static>> {
    prefixed_rows(
        text_image_rows(image, size, image_background_rgb(context.focused)),
        context,
    )
}

pub(in crate::source) fn render_popup_image(
    image: &InlineImage,
    id: ImageId,
    max_width: u16,
    max_height: u16,
    graphics: &mut GraphicsFrame,
) -> Vec<Line<'static>> {
    let unicode_limit = graphics
        .protocol()
        .uses_unicode_placeholders()
        .then_some(32);
    let size = fit_cells(
        u32::from(image.width()),
        u32::from(image.height()),
        CellBounds {
            columns: unicode_limit.map_or(max_width, |limit| max_width.min(limit)),
            rows: unicode_limit.map_or(max_height, |limit| max_height.min(limit)),
        },
    );
    let rows = if graphics.protocol().uses_placements() {
        graphics.push(id, image, size);
        native_image_rows(graphics.protocol(), id, size, image_background(true))
    } else {
        text_image_rows(image, size, image_background_rgb(true))
    };
    rows.into_iter().map(Line::from).collect()
}

fn native_image_rows(
    protocol: rasterm::Protocol,
    id: ImageId,
    size: CellSize,
    background: Color,
) -> Vec<Vec<Span<'static>>> {
    let foreground = graphics::image_color(id);
    (0..size.rows)
        .map(|row| {
            (0..size.columns)
                .map(|column| {
                    let symbol = if protocol.uses_unicode_placeholders() {
                        unicode_placeholder(row, column)
                            .expect("image placeholders stay inside Kitty's coordinate limit")
                    } else {
                        " ".to_owned()
                    };
                    Span::styled(symbol, Style::default().fg(foreground).bg(background))
                })
                .collect()
        })
        .collect()
}

fn text_image_rows(
    image: &InlineImage,
    size: CellSize,
    background: (u8, u8, u8),
) -> Vec<Vec<Span<'static>>> {
    let image = Image::from_rgba(
        u32::from(image.width()),
        u32::from(image.height()),
        image.rgba().to_vec(),
    )
    .expect("an Intuigram InlineImage has already validated its RGBA dimensions");
    let cells = text_cells(&image, size, background);
    (0..size.rows)
        .map(|line| {
            let offset = usize::from(line) * usize::from(size.columns);
            cells[offset..offset + usize::from(size.columns)]
                .iter()
                .map(|cell| {
                    Span::styled(
                        "▀",
                        Style::default()
                            .fg(Color::Rgb(cell.upper.0, cell.upper.1, cell.upper.2))
                            .bg(Color::Rgb(cell.lower.0, cell.lower.1, cell.lower.2)),
                    )
                })
                .collect()
        })
        .collect()
}

fn prefixed_rows(rows: Vec<Vec<Span<'static>>>, context: ImageRenderContext) -> Vec<Line<'static>> {
    rows.into_iter()
        .map(|pixels| {
            let mut spans = Vec::with_capacity(pixels.len().saturating_add(1));
            spans.extend(context.component.prefix(context.active, context.selected));
            spans.extend(pixels);
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
