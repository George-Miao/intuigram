use super::*;

#[test]
fn storage_maintenance_requires_one_positive_account_id() {
    let parsed = parse_arguments(["--media-cache-usage".to_owned(), "42".to_owned()])
        .expect("valid maintenance arguments should parse");
    assert!(matches!(parsed.maintenance, Some(Maintenance::MediaUsage(id)) if id.get() == 42));

    assert!(parse_arguments(["--clear-media-cache".to_owned(), "0".to_owned()]).is_err());
    assert!(
        parse_arguments([
            "--clear-media-cache".to_owned(),
            "1".to_owned(),
            "--clear-account-data".to_owned(),
            "1".to_owned(),
        ])
        .is_err()
    );
}

#[test]
fn account_launcher_arguments_are_unambiguous() {
    let selected = parse_arguments(["--account".to_owned(), "42".to_owned()])
        .expect("Account selection should parse");
    assert_eq!(selected.account.map(|account| account.get()), Some(42));

    assert!(
        parse_arguments([
            "--account".to_owned(),
            "42".to_owned(),
            "--add-account".to_owned(),
        ])
        .is_err()
    );
}

#[test]
fn connection_test_is_explicit_and_cannot_mutate_storage() {
    let parsed =
        parse_arguments(["--test-connection".to_owned()]).expect("connection test should parse");
    assert!(parsed.test_connection);
    assert!(
        parse_arguments([
            "--test-connection".to_owned(),
            "--clear-media-cache".to_owned(),
            "42".to_owned(),
        ])
        .is_err()
    );
}

#[test]
fn folder_commands_parse_rules_and_reject_built_in_ids() {
    let parsed = parse_arguments([
        "--folder-create".to_owned(),
        "42".to_owned(),
        "Work".to_owned(),
        "contacts,groups,exclude-muted".to_owned(),
    ])
    .expect("valid Folder creation should parse");
    let Some(Maintenance::Folder(account, FolderMaintenance::Create { title, rules })) =
        parsed.maintenance
    else {
        panic!("Folder creation should retain its typed command");
    };
    assert_eq!(account.get(), 42);
    assert_eq!(title, "Work");
    assert!(rules.contacts && rules.groups && rules.exclude_muted);

    assert!(
        parse_arguments([
            "--folder-delete".to_owned(),
            "42".to_owned(),
            "0".to_owned(),
        ])
        .is_err()
    );
}

#[test]
fn recording_and_contact_commands_keep_typed_chat_targets() {
    let recorded = parse_arguments([
        "--record-media".to_owned(),
        "42".to_owned(),
        "-1001195461650".to_owned(),
        "voice".to_owned(),
        "15".to_owned(),
        ":0".to_owned(),
    ])
    .expect("voice recording should parse");
    assert!(matches!(
        recorded.maintenance,
        Some(Maintenance::RichMedia(
            account,
            RichMediaMaintenance::Record {
                chat,
                kind: UploadKind::Voice,
                seconds: 15,
                ..
            }
        )) if account.get() == 42 && chat.0 == -1_001_195_461_650
    ));

    assert!(
        parse_arguments([
            "--record-media".to_owned(),
            "42".to_owned(),
            "7".to_owned(),
            "sticker".to_owned(),
            "2".to_owned(),
            "0".to_owned(),
        ])
        .is_err()
    );
}

#[test]
fn scheduled_commands_keep_server_message_identity() {
    let parsed = parse_arguments([
        "--scheduled-reschedule".to_owned(),
        "42".to_owned(),
        "-1001195461650".to_owned(),
        "123".to_owned(),
        "2030-06-01T09:30:00+08:00".to_owned(),
    ])
    .expect("a Scheduled Message reschedule should parse");
    assert!(matches!(
        parsed.maintenance,
        Some(Maintenance::Scheduled(
            account,
            ScheduledMaintenance::Reschedule {
                chat: ChatId(-1_001_195_461_650),
                message: 123,
                ..
            }
        )) if account.get() == 42
    ));
}
