use super::*;

fn arguments(
    account: Option<&str>,
    test_connection: bool,
    command: Command,
) -> ArgumentResult<Arguments> {
    let global = Global::new(
        Directories::default(),
        account.map(str::to_owned),
        test_connection,
    )?;
    Arguments::new(global, command)
}

fn maintenance(arguments: Arguments) -> Maintenance {
    let Command::Maintenance(command) = arguments.command else {
        panic!("expected an Account maintenance command");
    };
    command.into_inner()
}

#[test]
fn maintenance_positive_account_parses() {
    let parsed = arguments(Some("42"), false, Command::cache_usage())
        .expect("valid maintenance arguments should parse");
    assert_eq!(parsed.global.account.map(AccountId::get), Some(42));
    assert!(matches!(maintenance(parsed), Maintenance::MediaUsage));
}

#[test]
fn maintenance_invalid_account_fails() {
    assert!(arguments(Some("0"), false, Command::cache_clear()).is_err());
    assert!(arguments(None, false, Command::cache_clear()).is_err());
}

#[test]
fn account_add_with_selector_fails() {
    assert!(arguments(Some("42"), false, Command::account_add()).is_err());
}

#[test]
fn connection_test_start_parses() {
    let parsed = arguments(None, true, Command::start()).expect("connection test should parse");
    assert!(parsed.global.test_connection);
}

#[test]
fn connection_test_mutation_fails() {
    assert!(arguments(Some("42"), true, Command::cache_clear()).is_err());
}

#[test]
fn folder_create_rules_parse() {
    let parsed = arguments(
        Some("42"),
        false,
        Command::folder_create(
            "Work".to_owned(),
            "contacts,groups,exclude-muted".to_owned(),
        )
        .expect("valid Folder creation should parse"),
    )
    .expect("the selected Account should be valid");
    let Maintenance::Folder(FolderMaintenance::Create { title, rules }) = maintenance(parsed)
    else {
        panic!("Folder creation should retain its typed command");
    };
    assert_eq!(title, "Work");
    assert!(rules.contacts && rules.groups && rules.exclude_muted);
}

#[test]
fn folder_delete_builtin_fails() {
    assert!(Command::folder_delete("0".to_owned()).is_err());
}

#[test]
fn media_record_fields_parse() {
    let parsed = arguments(
        Some("42"),
        false,
        Command::media_record(
            "-1001195461650".to_owned(),
            "voice".to_owned(),
            "15".to_owned(),
            ":0".to_owned(),
        )
        .expect("voice recording should parse"),
    )
    .expect("the selected Account should be valid");
    assert!(matches!(
        maintenance(parsed),
        Maintenance::RichMedia(RichMediaMaintenance::Record {
            chat,
            kind: UploadKind::Voice,
            seconds: 15,
            ..
        }) if chat.0 == -1_001_195_461_650
    ));
}

#[test]
fn media_record_sticker_fails() {
    assert!(
        Command::media_record(
            "7".to_owned(),
            "sticker".to_owned(),
            "2".to_owned(),
            "0".to_owned(),
        )
        .is_err()
    );
}

#[test]
fn scheduled_reschedule_identity_parses() {
    let parsed = arguments(
        Some("42"),
        false,
        Command::scheduled_reschedule(
            "-1001195461650".to_owned(),
            "123".to_owned(),
            "2030-06-01T09:30:00+08:00".to_owned(),
        )
        .expect("a Scheduled Message reschedule should parse"),
    )
    .expect("the selected Account should be valid");
    assert!(matches!(
        maintenance(parsed),
        Maintenance::Scheduled(ScheduledMaintenance::Reschedule {
            chat: ChatId(-1_001_195_461_650),
            message: 123,
            ..
        })
    ));
}
