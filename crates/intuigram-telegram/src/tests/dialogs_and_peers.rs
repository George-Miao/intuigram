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
            kind: ChatKind::Private,
            folders: vec![0, 2],
        },
        ChatView {
            id: ChatId(20),
            title: "Archived".to_owned(),
            preview: String::new(),
            unread: 2,
            pinned: false,
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
}

use super::*;
