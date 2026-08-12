use super::*;

impl Client {
    /// Loads richer Active-Chat membership metadata when Telegram exposes it.
    pub async fn chat_status(&mut self, chat: ChatId) -> Result<Option<String>> {
        let peer = self.peers.resolve(chat)?;
        let response = match peer {
            tl::enums::InputPeer::Chat(peer) => self
                .connection
                .invoke(&tl::functions::messages::GetFullChat {
                    chat_id: peer.chat_id,
                })
                .await
                .context(InvokeSnafu)?,
            tl::enums::InputPeer::Channel(peer) => self
                .connection
                .invoke(&tl::functions::channels::GetFullChannel {
                    channel: tl::types::InputChannel {
                        channel_id: peer.channel_id,
                        access_hash: peer.access_hash,
                    }
                    .into(),
                })
                .await
                .context(InvokeSnafu)?,
            tl::enums::InputPeer::Empty
            | tl::enums::InputPeer::PeerSelf
            | tl::enums::InputPeer::User(_)
            | tl::enums::InputPeer::UserFromMessage(_)
            | tl::enums::InputPeer::ChannelFromMessage(_) => return Ok(None),
        };
        let tl::enums::messages::ChatFull::Full(response) = response;
        let status = full_chat_status(chat, &response);
        self.update_peer_cache(&response.chats, &response.users);
        Ok(status)
    }
}

fn full_chat_status(chat: ChatId, response: &tl::types::messages::ChatFull) -> Option<String> {
    let kind = response
        .chats
        .iter()
        .find(|candidate| cloud_chat_id(candidate) == chat)
        .map(cloud_chat_kind)
        .unwrap_or(ChatKind::Supergroup);
    match &response.full_chat {
        tl::enums::ChatFull::ChannelFull(full) => member_status(
            full.participants_count,
            (!matches!(kind, ChatKind::Channel))
                .then_some(full.online_count)
                .flatten(),
            if matches!(kind, ChatKind::Channel) {
                "subscribers"
            } else {
                "members"
            },
        ),
        tl::enums::ChatFull::Full(full) => {
            let members = match &full.participants {
                tl::enums::ChatParticipants::Participants(participants) => {
                    Some(i32::try_from(participants.participants.len()).unwrap_or(i32::MAX))
                }
                tl::enums::ChatParticipants::Forbidden(_) => response
                    .chats
                    .iter()
                    .find_map(basic_group_participant_count),
            };
            let online = i32::try_from(
                response
                    .users
                    .iter()
                    .filter(|user| {
                        matches!(
                            user,
                            tl::enums::User::User(user)
                                if matches!(user.status, Some(tl::enums::UserStatus::Online(_)))
                        )
                    })
                    .count(),
            )
            .unwrap_or(i32::MAX);
            member_status(members, Some(online), "members")
        }
    }
}

fn cloud_chat_id(chat: &tl::enums::Chat) -> ChatId {
    match chat {
        tl::enums::Chat::Chat(chat) => ChatId(-chat.id),
        tl::enums::Chat::Forbidden(chat) => ChatId(-chat.id),
        tl::enums::Chat::Empty(chat) => ChatId(-chat.id),
        tl::enums::Chat::Channel(chat) => ChatId(mark_channel_id(chat.id)),
        tl::enums::Chat::ChannelForbidden(chat) => ChatId(mark_channel_id(chat.id)),
    }
}

fn basic_group_participant_count(chat: &tl::enums::Chat) -> Option<i32> {
    match chat {
        tl::enums::Chat::Chat(chat) => Some(chat.participants_count),
        tl::enums::Chat::Channel(chat) => chat.participants_count,
        tl::enums::Chat::Forbidden(_)
        | tl::enums::Chat::ChannelForbidden(_)
        | tl::enums::Chat::Empty(_) => None,
    }
}

fn member_status(members: Option<i32>, online: Option<i32>, noun: &str) -> Option<String> {
    let members = members?.max(0);
    let online = online.unwrap_or(0).max(0);
    Some(if online > 0 {
        format!("{members} {noun}, {online} online")
    } else {
        format!("{members} {noun}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn membership_status_includes_nonzero_presence() {
        assert_eq!(
            member_status(Some(240), Some(31), "members").as_deref(),
            Some("240 members, 31 online")
        );
        assert_eq!(
            member_status(Some(240), Some(0), "members").as_deref(),
            Some("240 members")
        );
        assert_eq!(member_status(None, Some(31), "members"), None);
    }
}
