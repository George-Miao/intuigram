use intuigram_app::{ChatKind, MessageId, TopicDraftView, TopicId, TopicView};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

fn topic(id: i64, title: &str, unread: u32) -> TopicView {
    TopicView {
        id: TopicId(id),
        title: title.to_owned(),
        preview: format!("latest in {title}"),
        timestamp: "12:00".to_owned(),
        unread,
        pinned: id == 40,
        closed: false,
        hidden: id == 1,
        icon_color: 0x6f_76_5b,
        icon_emoji_id: None,
        top_message: Some(MessageId(id + 1)),
        draft: (id == 40).then(|| TopicDraftView {
            text: "independent Topic Draft".to_owned(),
            reply_to: None,
        }),
    }
}

#[test]
fn opening_a_forum_descends_through_topics_and_returns_there() -> Result<()> {
    let mut forum = chat(10, "Intuigram Forum");
    forum.kind = ChatKind::Supergroup;
    forum.has_topics = true;
    let general = topic(1, "General", 0);
    let design = topic(40, "Design", 3);
    let mut message = incoming(41, "Lin", "Topic history");
    message.details.thread_root = Some(MessageId(40));

    let mut app = TestSystem::builder()
        .name("topics-navigation")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(forum))
                .expect_load_topics(10, [general, design])
                .expect_load_thread(10, 40, [message])
                .expect_read_thread(10, 40, 41),
        )
        .start()?;

    app.press(key::ENTER)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("General"))
    );
    assert!(app.screen().rows().iter().any(|row| row.contains("Design")));

    app.press(key::DOWN)?;
    app.press(key::ENTER)?;
    app.screen().composer().expect_focused()?;
    app.screen()
        .composer()
        .expect_text("independent Topic Draft")?;
    app.screen()
        .message_text("Topic history")
        .expect_sender("Lin")?;

    app.press(key::ESCAPE)?;
    assert!(app.screen().rows().iter().any(|row| row.contains("Design")));
    app.expect_no_unhandled_work()
}
