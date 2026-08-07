use url::Url;

use super::{LinkTarget, MessageView, TextEntityKind};

pub(crate) fn active_link(message: &MessageView) -> Option<LinkTarget> {
    message.details.entities.iter().find_map(|entity| {
        let display = utf16_slice(&message.body, entity.offset, entity.length)?;
        let url = match &entity.kind {
            TextEntityKind::Url => display.clone(),
            TextEntityKind::TextUrl { url } => url.clone(),
            _ => return None,
        };
        classify_link(display, url)
    })
}

fn classify_link(display: String, url: String) -> Option<LinkTarget> {
    let (parsed, destination) = match Url::parse(&url) {
        Ok(parsed) => (parsed, url),
        Err(_) => {
            let destination = format!("https://{url}");
            (Url::parse(&destination).ok()?, destination)
        }
    };
    if !matches!(parsed.scheme(), "http" | "https" | "tg") {
        return None;
    }
    let telegram_username = telegram_username(&parsed);
    if parsed.scheme() == "tg" && telegram_username.is_none() {
        return None;
    }
    let suspicious = displayed_host_mismatch(&display, &parsed)
        || parsed.scheme() == "http"
        || !parsed.username().is_empty()
        || parsed.password().is_some();
    Some(LinkTarget {
        display,
        url: destination,
        telegram_username,
        suspicious,
    })
}

fn telegram_username(url: &Url) -> Option<String> {
    if url.scheme() == "tg" && url.host_str() == Some("resolve") {
        return url
            .query_pairs()
            .find_map(|(key, value)| (key == "domain").then(|| value.into_owned()));
    }
    let telegram_host = url
        .host_str()
        .is_some_and(|host| matches!(host, "t.me" | "telegram.me" | "telegram.dog"));
    telegram_host
        .then(|| {
            url.path_segments()
                .and_then(|mut segments| segments.next())
                .map(|username| username.trim_start_matches('@').to_owned())
        })
        .flatten()
        .filter(|username| !username.is_empty() && !username.starts_with('+'))
}

fn displayed_host_mismatch(display: &str, target: &Url) -> bool {
    let Ok(displayed) = Url::parse(display) else {
        return false;
    };
    displayed.host_str() != target.host_str()
}

fn utf16_slice(text: &str, offset: usize, length: usize) -> Option<String> {
    let end = offset.checked_add(length)?;
    let mut units = 0;
    let mut start_byte = None;
    let mut end_byte = None;
    for (byte, character) in text.char_indices() {
        if units == offset {
            start_byte = Some(byte);
        }
        if units == end {
            end_byte = Some(byte);
            break;
        }
        units += character.len_utf16();
        if units > end || (units > offset && start_byte.is_none()) {
            return None;
        }
    }
    if units == offset && start_byte.is_none() {
        start_byte = Some(text.len());
    }
    if units == end && end_byte.is_none() {
        end_byte = Some(text.len());
    }
    Some(text.get(start_byte?..end_byte?)?.to_owned())
}

#[cfg(test)]
mod tests {
    use super::classify_link;

    #[test]
    fn telegram_usernames_are_routed_internally() {
        let link = classify_link("Intuigram".to_owned(), "https://t.me/intuigram".to_owned())
            .expect("Telegram URL should parse");
        assert_eq!(link.telegram_username.as_deref(), Some("intuigram"));
        assert!(!link.suspicious);
    }

    #[test]
    fn disguised_and_insecure_links_require_confirmation() {
        let disguised = classify_link(
            "https://example.com".to_owned(),
            "https://evil.example".to_owned(),
        )
        .expect("URL should parse");
        assert!(disguised.suspicious);

        let insecure = classify_link("HTTP".to_owned(), "http://example.com".to_owned())
            .expect("URL should parse");
        assert!(insecure.suspicious);

        assert!(
            classify_link("Local".to_owned(), "file:///tmp/payload".to_owned()).is_none(),
            "non-web schemes must never reach the platform launcher"
        );
    }
}
