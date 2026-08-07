use super::*;

pub(super) fn effort_spans(label: &str, frame: u8) -> Vec<Span<'static>> {
    let characters = label.chars().collect::<Vec<_>>();
    if characters.is_empty() {
        return Vec::new();
    }
    let highlight = usize::from(frame) % characters.len();
    characters
        .into_iter()
        .enumerate()
        .map(|(index, character)| {
            let style = if index == highlight {
                Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(MUTED_TEXT)
            };
            Span::styled(character.to_string(), style)
        })
        .collect()
}
