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
            unread: 0,
            pinned: false,
            kind: traits
                .get(&chat)
                .map_or(ChatKind::Inaccessible, |traits| traits.kind),
            folders: vec![0],
        })
    }
}
