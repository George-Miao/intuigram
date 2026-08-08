use crate::PLACEHOLDER;

const DIACRITICS: [char; 32] = [
    '\u{0305}', '\u{030d}', '\u{030e}', '\u{0310}', '\u{0312}', '\u{033d}', '\u{033e}', '\u{033f}',
    '\u{0346}', '\u{034a}', '\u{034b}', '\u{034c}', '\u{0350}', '\u{0351}', '\u{0352}', '\u{0357}',
    '\u{035b}', '\u{0363}', '\u{0364}', '\u{0365}', '\u{0366}', '\u{0367}', '\u{0368}', '\u{0369}',
    '\u{036a}', '\u{036b}', '\u{036c}', '\u{036d}', '\u{036e}', '\u{036f}', '\u{0483}', '\u{0484}',
];

/// Builds one Kitty Unicode placeholder cell for a zero-based row and column.
///
/// The caller conveys the image ID through the cell foreground color.
#[must_use]
pub fn unicode_placeholder(row: u16, column: u16) -> Option<String> {
    let row = *DIACRITICS.get(usize::from(row))?;
    let column = *DIACRITICS.get(usize::from(column))?;
    Some(format!("{PLACEHOLDER}{row}{column}"))
}

#[cfg(test)]
mod tests {
    use super::unicode_placeholder;

    #[test]
    fn coordinates_are_encoded_after_the_placeholder() {
        assert_eq!(
            unicode_placeholder(0, 1),
            Some("\u{10eeee}\u{0305}\u{030d}".to_owned())
        );
        assert_eq!(unicode_placeholder(32, 0), None);
    }
}
