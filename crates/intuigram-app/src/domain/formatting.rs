use super::{TextEntity, TextEntityKind};

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FormattedText {
    pub(crate) text: String,
    pub(crate) entities: Vec<TextEntity>,
}

pub(crate) fn format_markdown(input: &str) -> FormattedText {
    let mut formatted = FormattedText {
        text: String::new(),
        entities: Vec::new(),
    };
    parse_segment(input, &mut formatted);
    formatted
        .entities
        .sort_by_key(|entity| (entity.offset, entity.length));
    formatted
}

fn parse_segment(input: &str, formatted: &mut FormattedText) {
    let mut remaining = input;
    while !remaining.is_empty() {
        if let Some(escaped) = remaining.strip_prefix('\\') {
            if let Some(marker) = ["```", "**", "__", "~~", "||", "`", "_"]
                .into_iter()
                .find(|marker| escaped.starts_with(marker))
            {
                formatted.text.push_str(marker);
                remaining = &escaped[marker.len()..];
                continue;
            }
            if let Some(character) = escaped.chars().next() {
                formatted.text.push(character);
                remaining = &escaped[character.len_utf8()..];
                continue;
            }
        }
        if let Some(consumed) = parse_delimited(remaining, formatted) {
            remaining = &remaining[consumed..];
            continue;
        }
        if let Some(consumed) = parse_link(remaining, formatted) {
            remaining = &remaining[consumed..];
            continue;
        }
        let character = remaining
            .chars()
            .next()
            .expect("a non-empty string has a first character");
        formatted.text.push(character);
        remaining = &remaining[character.len_utf8()..];
    }
}

fn parse_delimited(input: &str, formatted: &mut FormattedText) -> Option<usize> {
    let (marker, kind, literal) = [
        ("```", TextEntityKind::Pre { language: None }, true),
        ("**", TextEntityKind::Bold, false),
        ("__", TextEntityKind::Underline, false),
        ("~~", TextEntityKind::Strike, false),
        ("||", TextEntityKind::Spoiler, false),
        ("`", TextEntityKind::Code, true),
        ("_", TextEntityKind::Italic, false),
    ]
    .into_iter()
    .find(|(marker, ..)| input.starts_with(marker))?;
    let content_start = marker.len();
    let content_end = input[content_start..].find(marker)? + content_start;
    if content_end == content_start {
        return None;
    }
    let offset = formatted.text.encode_utf16().count();
    let content = &input[content_start..content_end];
    let (content, kind) = if marker == "```" {
        content
            .split_once('\n')
            .map_or((content, kind), |(language, body)| {
                let language = (!language.is_empty()
                    && language.len() <= 32
                    && language.chars().all(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '_')
                    }))
                .then(|| language.to_owned());
                (body, TextEntityKind::Pre { language })
            })
    } else {
        (content, kind)
    };
    if literal {
        formatted.text.push_str(content);
    } else {
        parse_segment(content, formatted);
    }
    let length = formatted.text.encode_utf16().count().saturating_sub(offset);
    formatted.entities.push(TextEntity {
        offset,
        length,
        kind,
    });
    Some(content_end + marker.len())
}

fn parse_link(input: &str, formatted: &mut FormattedText) -> Option<usize> {
    let label_end = input.strip_prefix('[')?.find("](")? + 1;
    let url_start = label_end + 2;
    let url_end = input[url_start..].find(')')? + url_start;
    let label = &input[1..label_end];
    let url = &input[url_start..url_end];
    if label.is_empty() || url.is_empty() {
        return None;
    }
    let offset = formatted.text.encode_utf16().count();
    parse_segment(label, formatted);
    let length = formatted.text.encode_utf16().count().saturating_sub(offset);
    formatted.entities.push(TextEntity {
        offset,
        length,
        kind: TextEntityKind::TextUrl {
            url: url.to_owned(),
        },
    });
    Some(url_end + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_offsets_use_telegram_utf16_units() {
        let formatted = format_markdown("😀 **bold** and `code`");

        assert_eq!(formatted.text, "😀 bold and code");
        assert_eq!(formatted.entities[0].offset, 3);
        assert_eq!(formatted.entities[0].length, 4);
        assert_eq!(formatted.entities[1].offset, 12);
        assert_eq!(formatted.entities[1].length, 4);
    }

    #[test]
    fn unmatched_and_escaped_delimiters_remain_literal() {
        let formatted = format_markdown(r"\**literal\** and **open");

        assert_eq!(formatted.text, "**literal** and **open");
        assert!(formatted.entities.is_empty());
    }

    #[test]
    fn links_and_fenced_code_preserve_their_semantics() {
        let formatted = format_markdown("[site](https://example.com) ```rust\nfn main() {}```");

        assert_eq!(formatted.text, "site fn main() {}");
        assert!(matches!(
            &formatted.entities[0].kind,
            TextEntityKind::TextUrl { url } if url == "https://example.com"
        ));
        assert!(matches!(
            &formatted.entities[1].kind,
            TextEntityKind::Pre { language: Some(language) } if language == "rust"
        ));
    }
}
