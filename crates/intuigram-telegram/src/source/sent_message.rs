use super::*;

pub(super) fn sent_message_id(updates: tl::enums::Updates, random_id: i64) -> Result<MessageId> {
    let id = match updates {
        tl::enums::Updates::UpdateShortSentMessage(update) => Some(update.id),
        tl::enums::Updates::UpdateShortMessage(update) => Some(update.id),
        tl::enums::Updates::UpdateShortChatMessage(update) => Some(update.id),
        tl::enums::Updates::UpdateShort(update) => update_message_id(&update.update, random_id),
        tl::enums::Updates::Combined(updates) => updates
            .updates
            .iter()
            .find_map(|update| update_message_id(update, random_id)),
        tl::enums::Updates::Updates(updates) => updates
            .updates
            .iter()
            .find_map(|update| update_message_id(update, random_id)),
        tl::enums::Updates::TooLong => None,
    };
    id.map(|id| MessageId(i64::from(id)))
        .ok_or(Error::SentMessageIdentityUnavailable)
}

fn update_message_id(update: &tl::enums::Update, random_id: i64) -> Option<i32> {
    match update {
        tl::enums::Update::MessageId(update) if update.random_id == random_id => Some(update.id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_correlates_the_random_id_to_the_server_message() {
        let updates = updates_with_mapping(123);

        assert!(matches!(sent_message_id(updates, 123), Ok(MessageId(77))));
    }

    #[test]
    fn response_rejects_an_unrelated_random_id() {
        let updates = updates_with_mapping(456);

        assert!(matches!(
            sent_message_id(updates, 123),
            Err(Error::SentMessageIdentityUnavailable)
        ));
    }

    fn updates_with_mapping(random_id: i64) -> tl::enums::Updates {
        tl::types::Updates {
            updates: vec![tl::types::UpdateMessageId { id: 77, random_id }.into()],
            users: Vec::new(),
            chats: Vec::new(),
            date: 0,
            seq: 0,
        }
        .into()
    }
}
