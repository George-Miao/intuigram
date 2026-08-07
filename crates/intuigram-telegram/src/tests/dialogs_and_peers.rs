#[test]
fn dialog_filters_include_custom_and_shared_folders_in_server_order() {
    let title = |text: &str| {
        tl::types::TextWithEntities {
            text: text.to_owned(),
            entities: Vec::new(),
        }
        .into()
    };
    let filters = vec![
        tl::enums::DialogFilter::Default,
        tl::types::DialogFilter {
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
            title: title("Work"),
            emoticon: None,
            color: None,
            pinned_peers: Vec::new(),
            include_peers: Vec::new(),
            exclude_peers: Vec::new(),
        }
        .into(),
        tl::types::DialogFilterChatlist {
            has_my_invites: false,
            title_noanimate: false,
            id: 3,
            title: title("Shared"),
            emoticon: None,
            color: None,
            pinned_peers: Vec::new(),
            include_peers: Vec::new(),
        }
        .into(),
    ];

    let chats = vec![
        ChatView {
            id: ChatId(10),
            title: "Ada".to_owned(),
            preview: String::new(),
            unread: 5,
            pinned: false,
            can_pin_messages: true,
            kind: ChatKind::Private,
            folders: vec![0, 2],
        },
        ChatView {
            id: ChatId(20),
            title: "Archived".to_owned(),
            preview: String::new(),
            unread: 2,
            pinned: false,
            can_pin_messages: true,
            kind: ChatKind::Supergroup,
            folders: vec![-1, 3],
        },
    ];
    let folders = normalize_dialog_folders(filters, &chats);

    assert_eq!(
        folders
            .iter()
            .map(|folder| (folder.id, folder.title.as_str(), folder.unread))
            .collect::<Vec<_>>(),
        vec![
            (0, "All", 5),
            (2, "Work", 5),
            (3, "Shared", 2),
            (-1, "Archive", 2),
        ]
    );
}

#[test]
fn folder_membership_edit_overrides_rule_based_inclusion_explicitly() {
    let peer: tl::enums::InputPeer = tl::types::InputPeerUser {
        user_id: 7,
        access_hash: 9,
    }
    .into();
    let mut filter: tl::enums::DialogFilter = tl::types::DialogFilter {
        contacts: true,
        non_contacts: false,
        groups: false,
        broadcasts: false,
        bots: false,
        exclude_muted: false,
        exclude_read: false,
        exclude_archived: false,
        title_noanimate: false,
        id: 2,
        title: tl::types::TextWithEntities {
            text: "Work".to_owned(),
            entities: Vec::new(),
        }
        .into(),
        emoticon: None,
        color: None,
        pinned_peers: vec![peer.clone()],
        include_peers: Vec::new(),
        exclude_peers: Vec::new(),
    }
    .into();

    set_dialog_filter_membership(&mut filter, peer.clone(), false);
    {
        let tl::enums::DialogFilter::Filter(contents) = &filter else {
            panic!("ordinary filter fixture should remain ordinary")
        };
        assert!(!contents.pinned_peers.contains(&peer));
        assert!(!contents.include_peers.contains(&peer));
        assert_eq!(contents.exclude_peers, vec![peer.clone()]);
    }

    set_dialog_filter_membership(&mut filter, peer.clone(), true);
    let tl::enums::DialogFilter::Filter(contents) = &filter else {
        panic!("ordinary filter fixture should remain ordinary")
    };
    assert_eq!(contents.include_peers, vec![peer.clone()]);
    assert!(!contents.exclude_peers.contains(&peer));
}

#[test]
fn serialized_cloud_peers_cover_every_root_chat_kind() {
    let cases = [
        (
            tl::enums::User::User(user(1, true, false)).to_bytes(),
            Some(1),
            ChatKind::SavedMessages,
        ),
        (
            tl::enums::User::User(user(2, false, false)).to_bytes(),
            Some(1),
            ChatKind::Private,
        ),
        (
            tl::enums::User::User(user(3, false, true)).to_bytes(),
            Some(1),
            ChatKind::Bot,
        ),
        (
            tl::enums::User::Empty(tl::types::UserEmpty { id: 4 }).to_bytes(),
            Some(1),
            ChatKind::Inaccessible,
        ),
        (
            tl::enums::Chat::Chat(basic_group()).to_bytes(),
            Some(1),
            ChatKind::BasicGroup,
        ),
        (
            tl::enums::Chat::Channel(channel(false, false)).to_bytes(),
            Some(1),
            ChatKind::Supergroup,
        ),
        (
            tl::enums::Chat::Channel(channel(false, true)).to_bytes(),
            Some(1),
            ChatKind::Gigagroup,
        ),
        (
            tl::enums::Chat::Channel(channel(true, false)).to_bytes(),
            Some(1),
            ChatKind::Channel,
        ),
        (
            tl::enums::Chat::Forbidden(tl::types::ChatForbidden {
                id: 9,
                title: "Unavailable".to_owned(),
            })
            .to_bytes(),
            Some(1),
            ChatKind::Inaccessible,
        ),
    ];

    for (bytes, account_id, expected) in cases {
        assert_eq!(
            normalize_serialized_peer_kind(&bytes, account_id)
                .expect("current TL peer fixture should normalize"),
            expected
        );
    }
}

#[test]
fn live_update_exposes_operation_peers_for_new_root_chats() {
    let user_id = ChatId(206_899_663);
    let channel_id = ChatId(-1_001_195_461_650);
    let mut live_user = user(user_id.0, false, false);
    live_user.access_hash = Some(11);
    let mut live_channel = channel(false, false);
    live_channel.id = 1_195_461_650;
    live_channel.access_hash = Some(12);
    live_channel.creator = true;
    let update = tl::enums::Updates::Updates(tl::types::Updates {
        updates: Vec::new(),
        users: vec![live_user.into()],
        chats: vec![live_channel.into()],
        date: 1_700_000_000,
        seq: 1,
    });
    let mut names = HashMap::new();

    let normalized = normalize_live_update(&update.to_bytes(), &mut names)
        .expect("full live update should normalize");

    assert!(normalized.peers.resolve(channel_id).is_ok());
    assert!(normalized.peers.resolve(user_id).is_ok());
    assert!(normalized.events.iter().any(|event| matches!(
        event,
        AdapterEvent::ChatPinPermissionChanged {
            chat,
            can_pin_messages: true,
        } if *chat == channel_id
    )));
}

#[test]
fn minimal_channel_does_not_overwrite_cached_pin_permission() {
    let channel_id = ChatId(-1_000_000_000_006);
    let mut partial = channel(false, false);
    partial.min = true;
    partial.admin_rights = None;
    partial.default_banned_rights = None;
    let update = tl::enums::Updates::Updates(tl::types::Updates {
        updates: Vec::new(),
        users: Vec::new(),
        chats: vec![partial.into()],
        date: 1_700_000_000,
        seq: 1,
    });
    let mut names = HashMap::new();

    let normalized = normalize_live_update(&update.to_bytes(), &mut names)
        .expect("partial Channel update should normalize");

    assert!(!normalized.events.iter().any(|event| matches!(
        event,
        AdapterEvent::ChatPinPermissionChanged { chat, .. } if *chat == channel_id
    )));
}

#[test]
fn chat_traits_preserve_message_pin_permission() {
    let denied = channel(true, false);
    let denied_id = ChatId(-1_000_000_000_000 - denied.id);
    let mut creator = channel(true, false);
    creator.id = 8;
    creator.creator = true;
    let creator_id = ChatId(-1_000_000_000_000 - creator.id);
    let mut allowed_group = channel(false, false);
    allowed_group.id = 7;
    let allowed_group_id = ChatId(-1_000_000_000_000 - allowed_group.id);
    let mut denied_group = channel(false, false);
    denied_group.id = 9;
    denied_group.default_banned_rights = Some(banned_pin_rights().into());
    let denied_group_id = ChatId(-1_000_000_000_000 - denied_group.id);
    let mut minimal_group = channel(false, false);
    minimal_group.id = 10;
    minimal_group.min = true;
    let minimal_group_id = ChatId(-1_000_000_000_000 - minimal_group.id);

    let traits = chat_traits(
        &[
            denied.into(),
            creator.into(),
            allowed_group.into(),
            denied_group.into(),
            minimal_group.into(),
        ],
        &[user(7, false, false).into()],
        None,
    );

    assert!(!traits[&denied_id].can_pin_messages);
    assert!(traits[&creator_id].can_pin_messages);
    assert!(traits[&allowed_group_id].can_pin_messages);
    assert!(!traits[&denied_group_id].can_pin_messages);
    assert!(!traits[&minimal_group_id].can_pin_messages);
    assert!(traits[&ChatId(7)].can_pin_messages);
}

fn banned_pin_rights() -> tl::types::ChatBannedRights {
    tl::types::ChatBannedRights {
        view_messages: false,
        send_messages: false,
        send_media: false,
        send_stickers: false,
        send_gifs: false,
        send_games: false,
        send_inline: false,
        embed_links: false,
        send_polls: false,
        change_info: false,
        invite_users: false,
        pin_messages: true,
        manage_topics: false,
        send_photos: false,
        send_videos: false,
        send_roundvideos: false,
        send_audios: false,
        send_voices: false,
        send_docs: false,
        send_plain: false,
        edit_rank: false,
        send_reactions: false,
        until_date: 0,
    }
}

use super::*;
