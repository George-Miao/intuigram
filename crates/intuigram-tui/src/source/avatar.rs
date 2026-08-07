use super::*;

pub(super) fn avatar_badge(name: &str) -> Span<'static> {
    Span::styled(
        format!("[{}] ", avatar_initials(name)),
        Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
    )
}

fn avatar_initials(name: &str) -> String {
    let words = name
        .split_whitespace()
        .filter_map(|word| word.chars().find(|character| character.is_alphanumeric()))
        .collect::<Vec<_>>();
    let initials = match words.as_slice() {
        [] => vec!['?'],
        [_] => name
            .chars()
            .filter(|character| character.is_alphanumeric())
            .take(2)
            .collect(),
        [first, rest @ ..] => vec![
            *first,
            *rest.last().expect("multiple words have a last item"),
        ],
    };
    initials.into_iter().flat_map(char::to_uppercase).collect()
}

#[cfg(test)]
mod tests {
    use super::avatar_initials;

    #[test]
    fn initials_are_deterministic_for_words_unicode_and_empty_names() {
        assert_eq!(avatar_initials("Intuigram Team"), "IT");
        assert_eq!(avatar_initials("alice"), "AL");
        assert_eq!(avatar_initials("李 雷"), "李雷");
        assert_eq!(avatar_initials(""), "?");
    }
}
