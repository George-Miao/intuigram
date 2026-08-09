/// Maximum terminal-cell rectangle available to an image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellBounds {
    /// Maximum columns.
    pub columns: u16,

    /// Maximum rows.
    pub rows: u16,
}

/// Aspect-fitted terminal-cell size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellSize {
    /// Occupied columns.
    pub columns: u16,

    /// Occupied rows.
    pub rows: u16,
}

/// Pixel dimensions of one terminal cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellPixels {
    /// Cell width in pixels.
    pub width: u16,

    /// Cell height in pixels.
    pub height: u16,
}

impl Default for CellPixels {
    fn default() -> Self {
        Self {
            width: 8,
            height: 16,
        }
    }
}

impl CellPixels {
    /// Derives one cell's pixel size from a terminal's complete geometry.
    #[must_use]
    pub fn from_terminal(width: u16, height: u16, columns: u16, rows: u16) -> Option<Self> {
        let width = width.checked_div(columns)?;
        let height = height.checked_div(rows)?;
        (width > 0 && height > 0).then_some(Self { width, height })
    }
}

/// Fits pixel dimensions into terminal cells while accounting for cells being
/// approximately twice as tall as they are wide.
#[must_use]
pub fn fit_cells(width: u32, height: u32, bounds: CellBounds) -> CellSize {
    if width == 0 || height == 0 || bounds.columns == 0 || bounds.rows == 0 {
        return CellSize {
            columns: 0,
            rows: 0,
        };
    }

    let width = u64::from(width);
    let height = u64::from(height);
    let max_columns = u64::from(bounds.columns);
    let max_rows = u64::from(bounds.rows);
    let (columns, rows) = if width * max_rows * 2 <= height * max_columns {
        (rounded_ratio(width * max_rows * 2, height), max_rows)
    } else {
        (max_columns, rounded_ratio(height * max_columns, width * 2))
    };
    CellSize {
        columns: u16::try_from(columns.clamp(1, max_columns))
            .expect("the fitted width never exceeds a u16 cell bound"),
        rows: u16::try_from(rows.clamp(1, max_rows))
            .expect("the fitted height never exceeds a u16 cell bound"),
    }
}

const fn rounded_ratio(numerator: u64, denominator: u64) -> u64 {
    numerator.saturating_add(denominator / 2) / denominator
}

#[cfg(test)]
mod tests {
    use super::{CellBounds, CellPixels, CellSize, fit_cells};

    const BOUNDS: CellBounds = CellBounds {
        columns: 32,
        rows: 6,
    };

    #[test]
    fn aspect_fit_does_not_reserve_a_fixed_width_canvas() {
        assert_eq!(
            fit_cells(48, 96, BOUNDS),
            CellSize {
                columns: 6,
                rows: 6
            }
        );
        assert_eq!(
            fit_cells(96, 48, BOUNDS),
            CellSize {
                columns: 24,
                rows: 6
            }
        );
        assert_eq!(
            fit_cells(1, 1, BOUNDS),
            CellSize {
                columns: 12,
                rows: 6
            }
        );
    }

    #[test]
    fn terminal_pixel_extent_is_converted_to_one_cell() {
        assert_eq!(
            CellPixels::from_terminal(1_600, 960, 200, 60),
            Some(CellPixels {
                width: 8,
                height: 16,
            })
        );
        assert_eq!(CellPixels::from_terminal(0, 960, 200, 60), None);
    }
}
