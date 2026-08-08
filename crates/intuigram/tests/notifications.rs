use intuigram_app::{AdapterEvent, ChatId};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming};

#[test]
fn muted_chats_do_not_emit_notifications() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("notifications-respect-chat-mute")
        .telegram(
            TelegramScenario::new().bootstrap(
                account("Ada")
                    .with_chat(chat(10, "Quiet Chat"))
                    .with_muted_chat(10),
            ),
        )
        .start()?;

    app.telegram().inject(AdapterEvent::MessageAdded {
        chat: ChatId(10),
        message: Box::new(incoming(40, "Lin", "quiet update")),
    })?;
    assert!(app.notifications().is_empty());

    app.telegram().inject(AdapterEvent::ChatMuteChanged {
        chat: ChatId(10),
        muted: false,
    })?;
    app.telegram().inject(AdapterEvent::MessageAdded {
        chat: ChatId(10),
        message: Box::new(incoming(41, "Lin", "audible update")),
    })?;
    assert_eq!(app.notifications(), &[ChatId(10)]);
    app.expect_no_unhandled_work()
}
