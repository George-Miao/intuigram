use intuigram_app::InlineImage;

use super::*;
use crate::source::graphics::PLACEHOLDER;

const WIDTH: u16 = 32;
const HEIGHT: u16 = 6;
const DIACRITICS: [char; 32] = [
    '\u{0305}', '\u{030d}', '\u{030e}', '\u{0310}', '\u{0312}', '\u{033d}', '\u{033e}', '\u{033f}',
    '\u{0346}', '\u{034a}', '\u{034b}', '\u{034c}', '\u{0350}', '\u{0351}', '\u{0352}', '\u{0357}',
    '\u{035b}', '\u{0363}', '\u{0364}', '\u{0365}', '\u{0366}', '\u{0367}', '\u{0368}', '\u{0369}',
    '\u{036a}', '\u{036b}', '\u{036c}', '\u{036d}', '\u{036e}', '\u{036f}', '\u{0483}', '\u{0484}',
];

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
    if graphics.protocol() == GraphicsProtocol::KittyUnicode
        && let Some(id) = context.id
    {
        render_unicode_image(image, id, context, graphics)
    } else {
        render_text_image(image, context)
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

fn render_unicode_image(
    image: &InlineImage,
    id: u32,
    context: ImageRenderContext,
    graphics: &mut GraphicsFrame,
) -> Vec<Line<'static>> {
    let rows = HEIGHT.min(context.max_height);
    graphics.push(GraphicsRequest {
        id,
        image: image.clone(),
        columns: WIDTH,
        rows,
        x: 0,
        y: 0,
    });
    let foreground = Color::Rgb(
        u8::try_from((id >> 16) & 0xff).expect("a masked image ID byte fits in u8"),
        u8::try_from((id >> 8) & 0xff).expect("a masked image ID byte fits in u8"),
        u8::try_from(id & 0xff).expect("a masked image ID byte fits in u8"),
    );
    (0..rows)
        .map(|row| {
            let mut spans = Vec::with_capacity(usize::from(WIDTH).saturating_add(1));
            spans.extend(content_prefix(
                context.active,
                context.selected,
                context.forwarded,
            ));
            spans.extend((0..WIDTH).map(|column| {
                let symbol = format!(
                    "{PLACEHOLDER}{}{}",
                    DIACRITICS[usize::from(row)],
                    DIACRITICS[usize::from(column)],
                );
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

fn render_text_image(image: &InlineImage, context: ImageRenderContext) -> Vec<Line<'static>> {
    let background = image_background_rgb(context.focused);
    (0..HEIGHT.min(context.max_height))
        .map(|line| {
            let upper_y = line.saturating_mul(2);
            let mut spans = Vec::with_capacity(usize::from(WIDTH).saturating_add(1));
            spans.extend(content_prefix(
                context.active,
                context.selected,
                context.forwarded,
            ));
            spans.extend((0..WIDTH).map(|x| {
                if x >= image.width() || upper_y >= image.height() {
                    return Span::styled(
                        " ",
                        Style::default().bg(image_background(context.focused)),
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
