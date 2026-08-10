use intuigram_lib::{
    ChatId, ChatKind, DeliveryState, MessageId, SavedDialogDraftView, SavedDialogView,
};
use test_harness::{
    Result, TelegramScenario, TestSystem, account, chat, incoming, key, sent_message,
};

#[test]
fn managed_monoforum_opens_reads_and_sends_in_one_peer_dialog() -> Result<()> {
    let mut channel = chat(-100, "Broadcast inbox");
    channel.kind = ChatKind::Channel;
    channel.has_direct_messages = true;
    let dialog = SavedDialogView {
        peer: ChatId(20),
        title: "Ada".to_owned(),
        preview: "Can you help?".to_owned(),
        timestamp: "12:00".to_owned(),
        unread: 1,
        unread_mark: false,
        pinned: false,
        top_message: MessageId(7),
        draft: Some(SavedDialogDraftView {
            text: "Sure — ".to_owned(),
            reply_to: None,
        }),
    };
    let mut message = incoming(7, "Ada", "Can you help?");
    message.delivery = DeliveryState::Sent;
    message.details.saved_peer = Some(ChatId(20));

    let mut app = TestSystem::builder()
        .name("managed-monoforum-direct-message")
        .terminal(100, 26)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Channel admin").with_chat(channel))
                .expect_load_saved_dialogs(-100, [dialog])
                .expect_load_saved_history(-100, 20, [message])
                .expect_read_saved_history(-100, 20, 7)
                .hold_send_in_saved_dialog("answer", -100, 20, "Sure — happy to help"),
        )
        .start()?;

    app.press(key::ENTER)?;
    assert!(app.screen().rows().iter().any(|row| row.contains("Ada")));
    app.press(key::ENTER)?;
    app.screen()
        .message_text("Can you help?")
        .expect_sender("Ada")?;
    app.screen().composer().expect_text("Sure — ")?;

    app.type_text("happy to help")?;
    app.press(key::ENTER)?;
    app.screen()
        .message_text("Sure — happy to help")
        .expect_delivery(DeliveryState::Pending)?;

    app.telegram()
        .complete_saved("answer", ChatId(20), sent_message(8, ""))?;
    app.screen()
        .message(8)
        .expect_delivery(DeliveryState::Sent)?;
    app.press(key::ESCAPE)?;
    assert!(app.screen().rows().iter().any(|row| row.contains("Ada")));
    app.expect_no_unhandled_work()
}

#[test]
fn ordinary_channel_does_not_enter_direct_message_navigation() -> Result<()> {
    let mut channel = chat(-100, "Public broadcast");
    channel.kind = ChatKind::Channel;

    let mut app = TestSystem::builder()
        .name("ordinary-channel-remains-ordinary")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Reader").with_chat(channel))
                .expect_load_history(-100, []),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.screen().composer().expect_text("")?;
    app.expect_no_unhandled_work()
}
