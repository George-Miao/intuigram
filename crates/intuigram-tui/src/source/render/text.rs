use ratatui::text::Line;

pub(crate) fn capped_text(text: &str, max_width: usize) -> String {
    if Line::from(text).width() <= max_width {
        return text.to_owned();
    }
    const SUFFIX: &str = "...";
    if max_width <= SUFFIX.len() {
        return ".".repeat(max_width);
    }
    let content_width = max_width - SUFFIX.len();
    let mut result = String::new();
    let mut width = 0_usize;
    for character in text.chars() {
        let character_width = Line::from(character.to_string()).width();
        if width.saturating_add(character_width) > content_width {
            break;
        }
        result.push(character);
        width = width.saturating_add(character_width);
    }
    result.push_str(SUFFIX);
    result
}
