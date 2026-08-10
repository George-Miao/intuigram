use super::*;

#[test]
fn group_chat_rows_show_last_sender_preview_and_message_time() -> Result<()> {
    let mut group = chat(10, "Intuigram Team");
    group.kind = ChatKind::Supergroup;
    group.preview = "daily driver".to_owned();
    group.preview_sender = Some("Lin Qiao".to_owned());
    group.preview_sender_peer = Some(intuigram_lib::ChatId(20));
    group.preview_timestamp = "12:34".to_owned();
    let mut app = TestSystem::builder()
        .name("layout-group-chat-row")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(group)
                        .with_avatar(10)
                        .with_avatar(20),
                )
                .expect_avatar(10),
        )
        .start()?;

    let rows = app.screen().rows();
    let title_row = row_within(&rows, "Intuigram Team", 0, 32);
    let preview_row = row_within(&rows, "daily driver", 0, 32);

    assert!(row_segment(&rows, title_row, 0, 32).contains("12:34"));
    assert!(row_segment(&rows, title_row, 0, 32).contains('▀'));
    assert!(row_segment(&rows, preview_row, 0, 32).contains('▀'));
    assert!(!row_segment(&rows, preview_row, 0, 32).contains("[LQ]"));
    assert!(row_segment(&rows, preview_row, 0, 32).contains("Lin Qiao: daily driver"));
    app.expect_no_unhandled_work()
}

#[test]
fn transcript_sender_avatar_uses_two_row_message_layout() -> Result<()> {
    let mut group = chat(10, "Intuigram Team");
    group.kind = ChatKind::Supergroup;
    let mut message = incoming(40, "Lin Qiao", "daily driver");
    message.details.sender_peer = Some(intuigram_lib::ChatId(20));
    let mut app = TestSystem::builder()
        .name("layout-transcript-sender-avatar")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(group)
                        .with_avatar(20)
                        .with_history([message.clone()]),
                )
                .expect_avatar(20)
                .expect_load_history(10, [message]),
        )
        .start()?;

    app.press(key::ENTER)?;
    let rows = app.screen().rows();
    let heading = row_within(&rows, "Lin Qiao", 33, 100);
    let body = row_within(&rows, "daily driver", 33, 100);

    assert_eq!(body, heading + 1);
    assert!(row_segment(&rows, heading, 33, 100).contains('▀'));
    assert!(!row_segment(&rows, heading, 33, 100).contains("[LQ]"));
    assert!(
        row_segment(&rows, body, 33, 100)
            .find("daily driver")
            .is_some_and(|column| column >= 6)
    );
    app.expect_no_unhandled_work()
}

#[test]
fn active_chat_avatar_spans_the_title_and_status_rows() -> Result<()> {
    let mut group = chat(10, "rust.tw");
    group.kind = ChatKind::Supergroup;
    group.status = "1181 members".to_owned();
    let mut app = TestSystem::builder()
        .name("layout-active-chat-header-avatar")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(group).with_avatar(10))
                .expect_avatar(10)
                .expect_load_history(10, []),
        )
        .start()?;

    app.press(key::ENTER)?;
    let rows = app.screen().rows();
    let title = row_within(&rows, "rust.tw", 33, 100);
    let status = row_within(&rows, "1181 members", 33, 100);

    assert_eq!(status, title + 1);
    assert!(row_segment(&rows, title, 33, 100).contains('▀'));
    assert!(row_segment(&rows, status, 33, 100).contains('▀'));
    app.expect_no_unhandled_work()
}
