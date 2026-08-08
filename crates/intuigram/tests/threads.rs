use intuigram_app::{AdapterEvent, ChatId, ChatKind, DeliveryState, MessageId};
use intuigram_telegram::UpdateCursor;
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn channel_comment_thread_preserves_independent_draft_and_live_history() -> Result<()> {
    let mut channel = chat(10, "Intuigram News");
    channel.kind = ChatKind::Channel;

    let root = incoming(40, "Intuigram", "Release notes");
    let mut first_comment = incoming(41, "Lin", "Nice release");
    first_comment.reply_to = Some(MessageId(40));
    first_comment.details.thread_root = Some(MessageId(40));

    let mut app = TestSystem::builder()
        .name("threads-channel-comments")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(channel))
                .expect_load_history(10, [root])
                .expect_load_thread(10, 40, [first_comment.clone()])
                .expect_read_thread(10, 40, 41)
                .expect_read_thread(10, 40, 42)
                .expect_load_thread(10, 40, [first_comment])
                .expect_read_thread(10, 40, 42),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.type_text("parent draft")?;
    app.press(key::ALT_UP)?;
    app.choose_action("Open Thread")?;

    app.screen().message(41).expect_sender("Lin")?;
    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("")?;

    app.type_text("thread draft")?;
    app.expect_saved_thread_draft(10, 40, "thread draft")?;

    let mut live_comment = incoming(42, "Mina", "Following live");
    live_comment.reply_to = Some(MessageId(40));
    live_comment.details.thread_root = Some(MessageId(40));
    app.telegram().inject_update(
        UpdateCursor {
            pts: Some(20),
            date: Some(1_786_000_001),
            seq: Some(8),
            seq_start: Some(8),
            ..UpdateCursor::default()
        },
        AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(live_comment),
        },
    )?;

    app.screen().message(42).expect_sender("Mina")?;
    app.expect_durable_thread_message(10, 42, 40, "Following live")?;

    app.press(key::ESCAPE)?;
    app.screen().message(40).expect_sender("Intuigram")?;
    app.screen().composer().expect_text("parent draft")?;

    app.press(key::ALT_UP)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Open Thread")?;
    app.screen().composer().expect_text("thread draft")?;
    app.screen().message(42).expect_sender("Mina")?;
    app.expect_no_unhandled_work()
}

#[test]
fn reply_sent_from_channel_comments_stays_in_that_thread() -> Result<()> {
    let mut channel = chat(10, "Intuigram News");
    channel.kind = ChatKind::Channel;

    let root = incoming(40, "Intuigram", "Release notes");
    let mut comment = incoming(41, "Lin", "Nice release");
    comment.reply_to = Some(MessageId(40));
    comment.details.thread_root = Some(MessageId(40));

    let mut app = TestSystem::builder()
        .name("threads-send-channel-comment")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(channel))
                .expect_load_history(10, [root])
                .expect_load_thread(10, 40, [comment])
                .expect_read_thread(10, 40, 41)
                .hold_send_in_thread("reply", 10, 40, "Thanks", Some(41)),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Open Thread")?;
    app.press(key::ALT_UP)?;
    app.choose_action("Reply")?;
    app.type_text("Thanks")?;
    app.press(key::ENTER)?;

    app.screen()
        .message_text("Thanks")
        .expect_delivery(DeliveryState::Pending)?;
    app.telegram()
        .complete("reply", test_harness::sent_message(43, "Thanks"))?;
    app.screen()
        .message(43)
        .expect_delivery(DeliveryState::Sent)?;
    app.expect_no_unhandled_work()
}
