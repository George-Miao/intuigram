impl Client {
    /// Loads all root dialogs and normalizes them into application-owned data
    /// without leaking Telegram TL values.
    pub async fn bootstrap(&mut self, page_size: i32) -> Result<Bootstrap> {
        let dialog_filters = self
            .connection
            .invoke(&tl::functions::messages::GetDialogFilters {})
            .await
            .context(InvokeSnafu)?;
        let DialogBatch {
            dialogs,
            messages,
            chats,
            users,
        } = self.load_all_dialogs(page_size).await?;
        self.channel_pts.clear();
        for dialog in &dialogs {
            let tl::enums::Dialog::Dialog(dialog) = dialog else {
                continue;
            };
            let (tl::enums::Peer::Channel(peer), Some(pts)) = (&dialog.peer, dialog.pts) else {
                continue;
            };
            self.channel_pts
                .insert(ChatId(mark_channel_id(peer.channel_id)), pts);
        }
        let traits = chat_traits(
            &chats,
            &users,
            self.identity.as_ref().map(|identity| identity.id),
        );
        let tl::enums::messages::DialogFilters::Filters(dialog_filters) = dialog_filters;
        let top_messages: HashMap<(ChatId, i32), &tl::enums::Message> = messages
            .iter()
            .map(|message| ((message_chat_id(message), message.id()), message))
            .collect();
        let unix_time = time::OffsetDateTime::now_utc().unix_timestamp();
        let notification_defaults = self.notification_defaults(unix_time).await?;
        let muted_chats = dialogs
            .iter()
            .filter_map(|dialog| {
                let tl::enums::Dialog::Dialog(dialog) = dialog else {
                    return None;
                };
                let chat = marked_peer_id(&dialog.peer);
                let inherited = traits
                    .get(&chat)
                    .is_some_and(|traits| notification_defaults.muted(traits.kind));
                notifications_muted_at(&dialog.notify_settings, unix_time, inherited)
                    .then(|| marked_peer_id(&dialog.peer))
            })
            .collect();
        let chat_views = dialogs
            .iter()
            .filter_map(|dialog| match dialog {
                tl::enums::Dialog::Dialog(dialog) => {
                    let chat_id = marked_peer_id(&dialog.peer);
                    let title = self
                        .names
                        .get(&chat_id)
                        .cloned()
                        .unwrap_or_else(|| "Inaccessible peer".to_owned());
                    let (preview, preview_sender, preview_timestamp) = top_messages
                        .get(&(chat_id, dialog.top_message))
                        .map_or_else(
                            || (String::new(), None, String::new()),
                            |message| dialog_message_summary(message, &self.names),
                        );
                    Some(ChatView {
                        id: chat_id,
                        title,
                        preview,
                        preview_sender,
                        preview_timestamp,
                        status: traits.get(&chat_id).map_or_else(
                            || "unavailable".to_owned(),
                            |traits| traits.status.clone(),
                        ),
                        unread: u32::try_from(dialog.unread_count.max(0)).unwrap_or(0),
                        pinned: dialog.pinned,
                        can_pin_messages: traits
                            .get(&chat_id)
                            .is_some_and(|traits| traits.can_pin_messages),
                        kind: traits
                            .get(&chat_id)
                            .map_or(ChatKind::Inaccessible, |traits| traits.kind),
                        folders: dialog_folder_membership(
                            dialog,
                            &dialog_filters.filters,
                            traits.get(&chat_id),
                        ),
                    })
                }
                tl::enums::Dialog::Folder(_) => None,
            })
            .collect::<Vec<_>>();
        let initial_messages = match chat_views.first() {
            Some(chat) => self.history(chat.id, 60).await?,
            None => Vec::new(),
        };
        let account_name = self
            .identity
            .as_ref()
            .map_or_else(|| "Telegram".to_owned(), |user| user.display_name.clone());
        let folder_details = normalize_dialog_folder_details(&dialog_filters.filters);
        let folders = normalize_dialog_folders(dialog_filters.filters, &chat_views);
        Ok(Bootstrap {
            connection: intuigram_app::ConnectionState::Connected,
            account_name,
            notification_identity: self.identity.as_ref().map_or_else(
                || "telegram:pending".to_owned(),
                |identity| format!("telegram:{}", identity.id),
            ),
            muted_chats,
            accounts: Vec::new(),
            restored_selection: None,
            transcript_anchors: Vec::new(),
            folders,
            folder_details,
            chats: chat_views,
            messages: initial_messages,
            pinned_messages: Vec::new(),
            drafts: Vec::new(),
            histories: Vec::new(),
        })
    }

    /// Reads Telegram's complete durable update cursor.
    pub async fn synchronization_cursors(&mut self) -> Result<Vec<UpdateCursor>> {
        let state = self
            .connection
            .invoke(&tl::functions::updates::GetState {})
            .await
            .context(InvokeSnafu)?;
        let tl::enums::updates::State::State(state) = state;
        let mut cursors = vec![UpdateCursor {
            pts: Some(state.pts),
            qts: Some(state.qts),
            date: Some(state.date),
            seq: Some(state.seq),
            ..UpdateCursor::default()
        }];
        let mut channels = self.channel_pts.iter().collect::<Vec<_>>();
        channels.sort_unstable_by_key(|(chat, _)| chat.0);
        cursors.extend(channels.into_iter().map(|(chat, pts)| UpdateCursor {
            scope: UpdateScope::Channel(*chat),
            pts: Some(*pts),
            ..UpdateCursor::default()
        }));
        Ok(cursors)
    }

    /// Adds or removes a Chat from Archive or a custom Telegram Folder.
    pub async fn set_chat_folder(
        &mut self,
        chat: ChatId,
        folder: i32,
        included: bool,
    ) -> Result<()> {
        let peer = self.peers.resolve(chat)?;
        if folder == -1 {
            self.connection
                .invoke(&tl::functions::folders::EditPeerFolders {
                    folder_peers: vec![
                        tl::types::InputFolderPeer {
                            peer,
                            folder_id: i32::from(included),
                        }
                        .into(),
                    ],
                })
                .await
                .context(InvokeSnafu)?;
            return Ok(());
        }

        let tl::enums::messages::DialogFilters::Filters(mut filters) = self
            .connection
            .invoke(&tl::functions::messages::GetDialogFilters {})
            .await
            .context(InvokeSnafu)?;
        let filter = filters
            .filters
            .iter_mut()
            .find(|candidate| dialog_filter_id(candidate) == Some(folder))
            .context(FolderUnavailableSnafu { folder_id: folder })?;
        set_dialog_filter_membership(filter, peer, included);
        let accepted = self
            .connection
            .invoke(&tl::functions::messages::UpdateDialogFilter {
                id: folder,
                filter: Some(filter.clone()),
            })
            .await
            .context(InvokeSnafu)?;
        if !accepted {
            return FolderUpdateRejectedSnafu.fail();
        }
        Ok(())
    }
}
use super::*;
