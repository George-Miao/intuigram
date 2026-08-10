use intuigram_lib::{ChatId, ChatKind, MessageId, SavedDialogView};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn opening_saved_messages_descends_through_origin_dialogs_and_returns_there() -> Result<()> {
    let mut saved = chat(100, "Saved Messages");
    saved.kind = ChatKind::SavedMessages;
    let dialog = SavedDialogView {
        peer: ChatId(200),
        title: "Intuigram Contributors".to_owned(),
        preview: "saved design note".to_owned(),
        timestamp: "12:00".to_owned(),
        unread: 0,
        unread_mark: false,
        pinned: true,
        top_message: MessageId(42),
        draft: None,
    };
    let mut message = incoming(42, "Lin", "saved design note");
    message.details.saved_peer = Some(ChatId(200));

    let mut app = TestSystem::builder()
        .name("saved-message-navigation")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(saved))
                .expect_load_saved_dialogs(100, [dialog])
                .expect_load_saved_history(100, 200, [message]),
        )
        .start()?;

    app.press(key::ENTER)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Intuigram Contributors"))
    );

    app.press(key::ENTER)?;
    app.screen()
        .message_text("saved design note")
        .expect_sender("Lin")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .all(|row| !row.contains("Type or paste a message"))
    );

    app.press(key::ESCAPE)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Intuigram Contributors"))
    );
    app.expect_no_unhandled_work()
}
