use super::*;

impl Client {
    /// Resolves a Telegram username to an internal root Chat.
    pub async fn resolve_username(&mut self, username: String) -> Result<ChatView> {
        let response = self
            .connection
            .invoke(&tl::functions::contacts::ResolveUsername {
                username,
                referer: None,
            })
            .await
            .context(InvokeSnafu)?;
        let tl::enums::contacts::ResolvedPeer::Peer(resolved) = response;
        let chat = marked_peer_id(&resolved.peer);
        let traits = chat_traits(
            &resolved.chats,
            &resolved.users,
            self.identity.as_ref().map(|identity| identity.id),
        );
        self.update_peer_cache(&resolved.chats, &resolved.users);
        Ok(ChatView {
            id: chat,
            title: self
                .names
                .get(&chat)
                .cloned()
                .unwrap_or_else(|| "Inaccessible peer".to_owned()),
            preview: String::new(),
            preview_sender: None,
            preview_sender_peer: None,
            preview_timestamp: String::new(),
            status: traits
                .get(&chat)
                .map_or_else(|| "unavailable".to_owned(), |traits| traits.status.clone()),
            unread: 0,
            pinned: false,
            can_pin_messages: traits
                .get(&chat)
                .is_some_and(|traits| traits.can_pin_messages),
            has_topics: traits.get(&chat).is_some_and(|traits| traits.has_topics),
            has_direct_messages: traits
                .get(&chat)
                .is_some_and(|traits| traits.has_direct_messages),
            kind: traits
                .get(&chat)
                .map_or(ChatKind::Inaccessible, |traits| traits.kind),
            folders: vec![0],
        })
    }
}
