use intuigram_lib::{Action, DeliveryState, MediaCard, MediaKind};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, key, sent_message};

#[test]
fn active_message_actions_are_grouped_in_one_selectable_popup() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("message-actions-popup")
        .terminal(100, 32)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [sent_message(41, "action target")]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.screen()
        .action(Action::OpenActions)
        .expect_available()?;
    for action in [
        Action::Reply,
        Action::Edit,
        Action::Delete,
        Action::Forward,
        Action::React,
    ] {
        app.screen().action(action).expect_unavailable()?;
    }
    app.type_text("a")?;

    let popup = app.screen().rows().join("\n");
    for label in [
        "Message Actions",
        "Reply",
        "Edit",
        "Delete",
        "Forward",
        "React",
        "Open Thread",
        "Pin / Unpin",
        "Select Message",
    ] {
        assert!(popup.contains(label), "missing {label:?} in {popup:?}");
    }

    app.press(key::ENTER)?;
    app.screen().composer().expect_focused()?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Reply to 41"))
    );
    app.expect_no_unhandled_work()
}

#[test]
fn saving_an_edit_returns_to_an_empty_composer() -> Result<()> {
    let mut edited = sent_message(41, "new text");
    edited.details.edited = true;
    let mut app = TestSystem::builder()
        .name("message-actions-edit")
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(chat(10, "Rust"))
                        .with_draft(10, "unfinished draft"),
                )
                .expect_load_history(10, [sent_message(41, "old text")])
                .expect_edit_message(10, 41, "new text", edited),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Edit")?;
    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("old text")?;
    let editing = app.screen().rows();
    let preview = editing
        .iter()
        .position(|row| row.contains("│ Edit · old text"))
        .expect("editing should show a one-line quoted preview");
    assert!(
        editing[preview - 1]
            .chars()
            .skip(33)
            .all(|cell| cell == ' ')
    );
    assert!(!editing.iter().any(|row| row.contains("Edit Message 41")));
    for _ in 0.."old text".chars().count() {
        app.press(key::BACKSPACE)?;
    }
    app.type_text("new text")?;
    app.press(key::ENTER)?;

    app.screen()
        .message_text("new text")
        .expect_delivery(DeliveryState::Sent)?;
    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("")?;
    app.expect_saved_draft(10, "")?;
    app.expect_no_unhandled_work()
}

#[test]
fn editing_a_captionless_photo_starts_with_an_empty_caption() -> Result<()> {
    let mut photo = sent_message(41, "[Photo] image");
    photo.details.media = Some(MediaCard {
        kind: MediaKind::Photo,
        title: "Photo".to_owned(),
        description: "image".to_owned(),
        details: Vec::new(),
        poll: None,
        specialized: None,
        remote_id: Some("photo:41".to_owned()),
    });
    let mut edited = photo.clone();
    edited.body = "new caption".to_owned();
    edited.details.edited = true;
    let mut app = TestSystem::builder()
        .name("message-actions-edit-photo-caption")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [photo])
                .expect_media_preview(10, 41)
                .expect_edit_message(10, 41, "new caption", edited),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Edit")?;

    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("")?;
    app.type_text("new caption")?;
    app.press(key::ENTER)?;

    app.screen()
        .message_text("new caption")
        .expect_delivery(DeliveryState::Sent)?;
    app.expect_no_unhandled_work()
}

#[test]
fn editing_a_photo_can_replace_the_media_without_inventing_a_caption() -> Result<()> {
    let mut photo = sent_message(41, "[Photo] image");
    photo.details.media = Some(MediaCard {
        kind: MediaKind::Photo,
        title: "Photo".to_owned(),
        description: "image".to_owned(),
        details: Vec::new(),
        poll: None,
        specialized: None,
        remote_id: Some("photo:41".to_owned()),
    });
    let mut edited = photo.clone();
    edited.body.clear();
    edited.details.edited = true;
    let mut app = TestSystem::builder()
        .name("message-actions-replace-photo")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [photo])
                .expect_media_preview(10, 41)
                .expect_edit_message_with_attachment(10, 41, "", "replacement.png", edited),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Edit")?;
    app.choose_action("Attach File")?;
    app.type_text("/tmp/replacement.png")?;
    app.press(key::ENTER)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("1 attachment(s)"))
    );
    app.press(key::ENTER)?;

    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("")?;
    app.expect_no_unhandled_work()
}

#[test]
fn cancelling_an_edit_clears_the_stale_draft_without_leaving_the_composer() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("message-actions-cancel-edit")
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(chat(10, "Rust"))
                        .with_draft(10, "stale draft"),
                )
                .expect_load_history(10, [sent_message(41, "old text")]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Edit")?;
    app.press(key::ESCAPE)?;

    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("")?;
    app.screen().message_text("stale draft").expect_absent()?;
    app.expect_saved_draft(10, "")?;
    app.expect_no_unhandled_work()
}

#[test]
fn deleting_a_message_requires_confirmation_and_removes_it_durably() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("message-actions-delete")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [sent_message(41, "remove me")])
                .expect_delete_messages(10, [41]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Delete")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Delete Message 41?"))
    );
    app.screen().message(41).expect_active()?;
    app.press(key::ENTER)?;

    app.screen().message(41).expect_absent()?;
    app.expect_no_durable_message(10, 41)?;
    app.expect_no_unhandled_work()
}

#[test]
fn message_selection_is_visible_and_survives_navigation_and_resize() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("message-actions-selection")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(
                    10,
                    [
                        sent_message(41, "first"),
                        sent_message(42, "second"),
                        sent_message(43, "third"),
                    ],
                ),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Select Message")?;
    app.press(key::UP)?;
    app.choose_action("Select Message")?;

    assert!(selected_message_is_visible(&app, "second"));
    assert!(selected_message_is_visible(&app, "third"));
    app.resize(120, 32)?;
    assert!(selected_message_is_visible(&app, "second"));
    assert!(selected_message_is_visible(&app, "third"));

    app.choose_action("Select Message")?;
    assert!(!selected_message_is_visible(&app, "second"));
    assert!(selected_message_is_visible(&app, "third"));
    app.press(key::ESCAPE)?;
    app.screen().composer().expect_focused()?;
    assert!(!selected_message_is_visible(&app, "third"));
    app.expect_no_unhandled_work()
}

#[test]
fn compatible_actions_apply_to_every_selected_message() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("message-actions-batch-delete")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(
                    10,
                    [
                        sent_message(41, "keep"),
                        sent_message(42, "remove second"),
                        sent_message(43, "remove third"),
                    ],
                )
                .expect_delete_messages(10, [42, 43]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Select Message")?;
    app.press(key::UP)?;
    app.choose_action("Select Message")?;
    app.choose_action("Delete")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Delete 2 Messages?"))
    );
    app.press(key::ENTER)?;

    app.screen().message(42).expect_absent()?;
    app.screen().message(43).expect_absent()?;
    app.expect_no_durable_message(10, 42)?;
    app.expect_no_durable_message(10, 43)?;
    assert!(!selected_message_is_visible(&app, "keep"));
    app.expect_no_unhandled_work()
}

fn selected_message_is_visible(app: &TestSystem, body: &str) -> bool {
    app.screen()
        .rows()
        .iter()
        .any(|row| row.contains("[✓]") && row.contains(body))
}
