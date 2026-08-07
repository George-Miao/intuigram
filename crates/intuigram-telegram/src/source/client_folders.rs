use super::*;

/// Telegram's category and exclusion rules for an ordinary custom Folder.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FolderRules {
    /// Include saved contacts.
    pub contacts: bool,
    /// Include private Chats with non-contacts.
    pub non_contacts: bool,
    /// Include groups and supergroups.
    pub groups: bool,
    /// Include Channels.
    pub broadcasts: bool,
    /// Include bots.
    pub bots: bool,
    /// Exclude muted Chats after applying inclusion rules.
    pub exclude_muted: bool,
    /// Exclude read Chats after applying inclusion rules.
    pub exclude_read: bool,
    /// Exclude archived Chats after applying inclusion rules.
    pub exclude_archived: bool,
}

impl Client {
    /// Creates an ordinary custom Folder and returns its Telegram identifier.
    pub async fn create_folder(&mut self, title: String, rules: FolderRules) -> Result<i32> {
        let filters = self.dialog_filters().await?;
        let id = (2..=255)
            .find(|id| {
                !filters
                    .iter()
                    .any(|filter| dialog_filter_id(filter) == Some(*id))
            })
            .context(FolderLimitReachedSnafu)?;
        let filter = tl::types::DialogFilter {
            contacts: rules.contacts,
            non_contacts: rules.non_contacts,
            groups: rules.groups,
            broadcasts: rules.broadcasts,
            bots: rules.bots,
            exclude_muted: rules.exclude_muted,
            exclude_read: rules.exclude_read,
            exclude_archived: rules.exclude_archived,
            title_noanimate: false,
            id,
            title: plain_title(title),
            emoticon: None,
            color: None,
            pinned_peers: Vec::new(),
            include_peers: Vec::new(),
            exclude_peers: Vec::new(),
        };
        self.update_folder(id, Some(filter.into())).await?;
        Ok(id)
    }

    /// Renames an existing custom or shared Folder without changing its rules.
    pub async fn rename_folder(&mut self, folder_id: i32, title: String) -> Result<()> {
        let mut filter = self.folder(folder_id).await?;
        match &mut filter {
            tl::enums::DialogFilter::Default => {
                return FolderUnavailableSnafu { folder_id }.fail();
            }
            tl::enums::DialogFilter::Filter(filter) => filter.title = plain_title(title),
            tl::enums::DialogFilter::Chatlist(filter) => filter.title = plain_title(title),
        }
        self.update_folder(folder_id, Some(filter)).await
    }

    /// Replaces an ordinary custom Folder's category and exclusion rules.
    pub async fn set_folder_rules(&mut self, folder_id: i32, rules: FolderRules) -> Result<()> {
        let mut filter = self.folder(folder_id).await?;
        apply_rules(&mut filter, rules)?;
        self.update_folder(folder_id, Some(filter)).await
    }

    /// Moves one custom Folder to a zero-based position among custom Folders.
    pub async fn reorder_folder(&mut self, folder_id: i32, position: usize) -> Result<()> {
        let mut order = self
            .dialog_filters()
            .await?
            .iter()
            .filter_map(dialog_filter_id)
            .collect::<Vec<_>>();
        let current = order
            .iter()
            .position(|id| *id == folder_id)
            .context(FolderUnavailableSnafu { folder_id })?;
        order.remove(current);
        order.insert(position.min(order.len()), folder_id);
        let accepted = self
            .connection
            .invoke(&tl::functions::messages::UpdateDialogFiltersOrder { order })
            .await
            .context(InvokeSnafu)?;
        if !accepted {
            return FolderUpdateRejectedSnafu.fail();
        }
        Ok(())
    }

    /// Deletes a custom Folder while leaving its Chats intact.
    pub async fn delete_folder(&mut self, folder_id: i32) -> Result<()> {
        self.folder(folder_id).await?;
        self.update_folder(folder_id, None).await
    }

    /// Exports a Telegram share link containing the Folder's explicit Chats.
    pub async fn share_folder(&mut self, folder_id: i32) -> Result<String> {
        let filter = self.folder(folder_id).await?;
        let (title, peers) = shareable_parts(&filter);
        let exported = self
            .connection
            .invoke(&tl::functions::chatlists::ExportChatlistInvite {
                chatlist: tl::types::InputChatlistDialogFilter {
                    filter_id: folder_id,
                }
                .into(),
                title,
                peers,
            })
            .await
            .context(InvokeSnafu)?;
        let exported: tl::types::chatlists::ExportedChatlistInvite = exported.into();
        let invite: tl::types::ExportedChatlistInvite = exported.invite.into();
        Ok(invite.url)
    }

    async fn dialog_filters(&mut self) -> Result<Vec<tl::enums::DialogFilter>> {
        let tl::enums::messages::DialogFilters::Filters(filters) = self
            .connection
            .invoke(&tl::functions::messages::GetDialogFilters {})
            .await
            .context(InvokeSnafu)?;
        Ok(filters.filters)
    }

    async fn folder(&mut self, folder_id: i32) -> Result<tl::enums::DialogFilter> {
        self.dialog_filters()
            .await?
            .into_iter()
            .find(|filter| dialog_filter_id(filter) == Some(folder_id))
            .context(FolderUnavailableSnafu { folder_id })
    }

    async fn update_folder(
        &mut self,
        folder_id: i32,
        filter: Option<tl::enums::DialogFilter>,
    ) -> Result<()> {
        let accepted = self
            .connection
            .invoke(&tl::functions::messages::UpdateDialogFilter {
                id: folder_id,
                filter,
            })
            .await
            .context(InvokeSnafu)?;
        if !accepted {
            return FolderUpdateRejectedSnafu.fail();
        }
        Ok(())
    }
}

fn plain_title(text: String) -> tl::enums::TextWithEntities {
    tl::types::TextWithEntities {
        text,
        entities: Vec::new(),
    }
    .into()
}

fn apply_rules(filter: &mut tl::enums::DialogFilter, rules: FolderRules) -> Result<()> {
    let tl::enums::DialogFilter::Filter(filter) = filter else {
        return FolderRulesUnavailableSnafu {
            folder_id: dialog_filter_id(filter).unwrap_or_default(),
        }
        .fail();
    };
    filter.contacts = rules.contacts;
    filter.non_contacts = rules.non_contacts;
    filter.groups = rules.groups;
    filter.broadcasts = rules.broadcasts;
    filter.bots = rules.bots;
    filter.exclude_muted = rules.exclude_muted;
    filter.exclude_read = rules.exclude_read;
    filter.exclude_archived = rules.exclude_archived;
    Ok(())
}

fn shareable_parts(filter: &tl::enums::DialogFilter) -> (String, Vec<tl::enums::InputPeer>) {
    match filter {
        tl::enums::DialogFilter::Default => ("All".to_owned(), Vec::new()),
        tl::enums::DialogFilter::Filter(filter) => {
            let mut peers = filter.pinned_peers.clone();
            for peer in &filter.include_peers {
                if !peers.contains(peer) {
                    peers.push(peer.clone());
                }
            }
            (text_with_entities(filter.title.clone()), peers)
        }
        tl::enums::DialogFilter::Chatlist(filter) => {
            let mut peers = filter.pinned_peers.clone();
            for peer in &filter.include_peers {
                if !peers.contains(peer) {
                    peers.push(peer.clone());
                }
            }
            (text_with_entities(filter.title.clone()), peers)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_edits_preserve_explicit_membership() {
        let included: tl::enums::InputPeer = tl::types::InputPeerUser {
            user_id: 7,
            access_hash: 9,
        }
        .into();
        let mut filter: tl::enums::DialogFilter = tl::types::DialogFilter {
            contacts: false,
            non_contacts: false,
            groups: false,
            broadcasts: false,
            bots: false,
            exclude_muted: false,
            exclude_read: false,
            exclude_archived: false,
            title_noanimate: false,
            id: 2,
            title: plain_title("People".to_owned()),
            emoticon: None,
            color: None,
            pinned_peers: Vec::new(),
            include_peers: vec![included.clone()],
            exclude_peers: Vec::new(),
        }
        .into();

        apply_rules(
            &mut filter,
            FolderRules {
                contacts: true,
                exclude_muted: true,
                ..FolderRules::default()
            },
        )
        .expect("ordinary Folder rules should be editable");

        let tl::enums::DialogFilter::Filter(filter) = filter else {
            panic!("ordinary Folder should remain ordinary");
        };
        assert!(filter.contacts);
        assert!(filter.exclude_muted);
        assert_eq!(filter.include_peers, vec![included]);
    }
}
