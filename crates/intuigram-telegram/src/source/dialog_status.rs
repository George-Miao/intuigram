use super::*;

pub(super) fn user_status(user: &tl::enums::User, kind: ChatKind) -> String {
    if kind == ChatKind::SavedMessages {
        return "personal cloud".to_owned();
    }
    if kind == ChatKind::Bot {
        return "bot".to_owned();
    }
    let Some(status) = (match user {
        tl::enums::User::User(user) => user.status.as_ref(),
        tl::enums::User::Empty(_) => None,
    }) else {
        return if kind == ChatKind::Inaccessible {
            "unavailable"
        } else {
            "offline"
        }
        .to_owned();
    };
    match status {
        tl::enums::UserStatus::Online(_) => "online".to_owned(),
        tl::enums::UserStatus::Offline(status) => last_seen_status(
            i64::from(status.was_online),
            time::OffsetDateTime::now_utc().unix_timestamp(),
        ),
        tl::enums::UserStatus::Recently(_) => "last seen recently".to_owned(),
        tl::enums::UserStatus::LastWeek(_) => "last seen within a week".to_owned(),
        tl::enums::UserStatus::LastMonth(_) => "last seen within a month".to_owned(),
        tl::enums::UserStatus::Empty => "offline".to_owned(),
    }
}

fn last_seen_status(was_online: i64, now: i64) -> String {
    let elapsed = now.saturating_sub(was_online).max(0);
    match elapsed {
        0..=59 => "last seen just now".to_owned(),
        60..=3_599 => format!("last seen {} min ago", elapsed / 60),
        3_600..=86_399 => format!("last seen {} h ago", elapsed / 3_600),
        _ => format!("last seen {} d ago", elapsed / 86_400),
    }
}

pub(super) fn cloud_chat_status(chat: &tl::enums::Chat) -> String {
    match chat {
        tl::enums::Chat::Chat(chat) => count_status(chat.participants_count, "members"),
        tl::enums::Chat::Channel(channel) => channel.participants_count.map_or_else(
            || {
                if channel.broadcast {
                    "channel".to_owned()
                } else {
                    "group".to_owned()
                }
            },
            |count| {
                count_status(
                    count,
                    if channel.broadcast {
                        "subscribers"
                    } else {
                        "members"
                    },
                )
            },
        ),
        tl::enums::Chat::Forbidden(_)
        | tl::enums::Chat::ChannelForbidden(_)
        | tl::enums::Chat::Empty(_) => "unavailable".to_owned(),
    }
}

fn count_status(count: i32, noun: &str) -> String {
    format!("{} {noun}", count.max(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_seen_text_uses_stable_human_units() {
        assert_eq!(last_seen_status(9_970, 10_000), "last seen just now");
        assert_eq!(last_seen_status(9_400, 10_000), "last seen 10 min ago");
        assert_eq!(last_seen_status(2_800, 10_000), "last seen 2 h ago");
        assert_eq!(last_seen_status(-76_400, 10_000), "last seen 1 d ago");
    }
}
