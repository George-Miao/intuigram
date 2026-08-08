use crate::{CellSize, Image};

/// Foreground/background colors for one Unicode upper-half-block cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextCell {
    /// Upper pixel, rendered as foreground.
    pub upper: (u8, u8, u8),

    /// Lower pixel, rendered as background.
    pub lower: (u8, u8, u8),
}

/// Samples an RGBA image into portable upper-half-block cells.
#[must_use]
pub fn text_cells(image: &Image, size: CellSize, background: (u8, u8, u8)) -> Vec<TextCell> {
    let mut cells = Vec::with_capacity(usize::from(size.columns) * usize::from(size.rows));
    for row in 0..size.rows {
        for column in 0..size.columns {
            cells.push(TextCell {
                upper: sample(image, size, column, row.saturating_mul(2), background),
                lower: sample(
                    image,
                    size,
                    column,
                    row.saturating_mul(2).saturating_add(1),
                    background,
                ),
            });
        }
    }
    cells
}

fn sample(
    image: &Image,
    size: CellSize,
    column: u16,
    row: u16,
    background: (u8, u8, u8),
) -> (u8, u8, u8) {
    let target_height = u32::from(size.rows).saturating_mul(2).max(1);
    let x = u32::from(column)
        .saturating_mul(image.width())
        .checked_div(u32::from(size.columns).max(1))
        .unwrap_or_default()
        .min(image.width().saturating_sub(1));
    let y = u32::from(row)
        .saturating_mul(image.height())
        .checked_div(target_height)
        .unwrap_or_default()
        .min(image.height().saturating_sub(1));
    let offset = ((y as usize) * (image.width() as usize) + (x as usize)) * 4;
    let rgba = &image.rgba()[offset..offset + 4];
    (
        blend(rgba[0], background.0, rgba[3]),
        blend(rgba[1], background.1, rgba[3]),
        blend(rgba[2], background.2, rgba[3]),
    )
}

fn blend(channel: u8, background: u8, alpha: u8) -> u8 {
    let alpha = u16::from(alpha);
    let blended = u16::from(channel) * alpha + u16::from(background) * (255 - alpha) + 127;
    u8::try_from(blended / 255).expect("blending two u8 channels produces one u8 channel")
}

#[cfg(test)]
mod tests {
    use super::text_cells;
    use crate::{CellSize, Image};

    #[test]
    fn fallback_resamples_to_exact_cell_geometry() {
        let image = Image::from_rgba(1, 1, vec![255, 0, 0, 255])
            .expect("fixture dimensions should match its pixels");
        let cells = text_cells(
            &image,
            CellSize {
                columns: 2,
                rows: 1,
            },
            (0, 0, 0),
        );
        assert_eq!(cells.len(), 2);
        assert!(cells.iter().all(|cell| cell.upper == (255, 0, 0)));
    }
}
