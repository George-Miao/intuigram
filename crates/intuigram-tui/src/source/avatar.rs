use rasterm::{CellSize, Image, ImageId, text_cells, unicode_placeholder};

use super::*;

pub(super) fn avatar_spans(
    view: &View,
    peer: Option<ChatId>,
    name: &str,
    id: Option<ImageId>,
    graphics: &mut GraphicsFrame,
    focused: bool,
) -> Vec<Span<'static>> {
    avatar_rows(view, peer, name, id, graphics, focused, 1)
        .pop()
        .expect("a one-row avatar always renders one row")
}

pub(super) fn avatar_block(
    view: &View,
    peer: Option<ChatId>,
    name: &str,
    id: Option<ImageId>,
    graphics: &mut GraphicsFrame,
    focused: bool,
) -> [Vec<Span<'static>>; 2] {
    let mut rows = avatar_rows(view, peer, name, id, graphics, focused, 2).into_iter();
    [
        rows.next().expect("a two-row avatar has a top row"),
        rows.next().expect("a two-row avatar has a bottom row"),
    ]
}

fn avatar_rows(
    view: &View,
    peer: Option<ChatId>,
    name: &str,
    id: Option<ImageId>,
    graphics: &mut GraphicsFrame,
    focused: bool,
    row_count: u16,
) -> Vec<Vec<Span<'static>>> {
    let columns = graphics.square_columns(row_count);
    let Some((image, id)) = peer
        .and_then(|peer| {
            view.avatars
                .iter()
                .find(|avatar| avatar.avatar.peer == peer)
        })
        .zip(id)
    else {
        let loading =
            peer.is_some_and(|peer| view.avatar_loads.iter().any(|avatar| avatar.peer == peer));
        let color = if loading {
            Color::Rgb(128, 128, 128)
        } else {
            avatar_tile_color(peer, name)
        };
        return unicode_tile_rows(row_count, columns, color);
    };
    let size = CellSize {
        columns,
        rows: row_count,
    };
    let mut rows = if graphics.protocol().uses_placements() {
        graphics.push(id, &image.image, size);
        let foreground = graphics::image_color(id);
        (0..row_count)
            .map(|row| {
                (0..columns)
                    .map(|column| {
                        let symbol = if graphics.protocol().uses_unicode_placeholders() {
                            unicode_placeholder(row, column).expect(
                                "avatar placeholders remain inside Kitty's coordinate limit",
                            )
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
        let cells = text_cells(&image, size, background);
        (0..row_count)
            .map(|row| {
                let start = usize::from(row) * usize::from(columns);
                cells[start..start + usize::from(columns)]
                    .iter()
                    .map(|cell| {
                        Span::styled(
                            "▀",
                            Style::default()
                                .fg(Color::Rgb(cell.upper.0, cell.upper.1, cell.upper.2))
                                .bg(Color::Rgb(cell.lower.0, cell.lower.1, cell.lower.2)),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    for spans in &mut rows {
        spans.push(Span::raw(" "));
    }
    rows
}

fn unicode_tile_rows(row_count: u16, columns: u16, color: Color) -> Vec<Vec<Span<'static>>> {
    let tile = Span::styled("█".repeat(usize::from(columns)), Style::default().fg(color));
    (0..row_count)
        .map(|_| vec![tile.clone(), Span::raw(" ")])
        .collect()
}

fn avatar_tile_color(peer: Option<ChatId>, name: &str) -> Color {
    const COLORS: [Color; 6] = [
        PRIMARY,
        SECONDARY,
        Color::Rgb(245, 125, 38),
        Color::Rgb(232, 104, 90),
        Color::Rgb(159, 116, 196),
        Color::Rgb(53, 167, 156),
    ];
    let mut hash = 2_166_136_261_u32;
    if let Some(peer) = peer {
        for byte in peer.0.to_le_bytes() {
            hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
        }
    } else {
        for byte in name.as_bytes() {
            hash = (hash ^ u32::from(*byte)).wrapping_mul(16_777_619);
        }
    }
    COLORS[hash as usize % COLORS.len()]
}

pub(super) fn avatar_width(graphics: &GraphicsFrame, row_count: u16) -> usize {
    usize::from(graphics.square_columns(row_count).saturating_add(1))
}
