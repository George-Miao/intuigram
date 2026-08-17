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
fn visible_avatar_queue_renders_loaded_peers() -> Result<()> {
    let sender_peers = 20..36;
    let messages = sender_peers
        .clone()
        .enumerate()
        .map(|(index, peer)| {
            let mut message = incoming(index as i64 + 1, format!("Sender {peer}"), "message");
            message.details.sender_peer = Some(intuigram_lib::ChatId(peer));
            message
        })
        .collect::<Vec<_>>();
    let mut group = chat(10, "Chat 10");
    group.kind = ChatKind::Supergroup;
    let account = sender_peers.clone().fold(
        account("Ada")
            .with_chat(group)
            .with_avatar(10)
            .with_history(messages),
        |account, peer| account.with_avatar(peer),
    );
    let telegram = (27..36).fold(
        TelegramScenario::new().bootstrap(account).expect_avatar(10),
        |scenario, peer| scenario.expect_avatar(peer),
    );
    let mut app = TestSystem::builder()
        .name("layout-visible-avatar-window")
        .terminal(100, 42)
        .telegram(telegram)
        .start()?;

    let rows = app.screen().rows();
    let title_row = row_within(&rows, "Chat 10", 0, 32);
    assert!(row_segment(&rows, title_row, 0, 32).contains('▀'));
    let sender_rows = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| row.contains("Sender ").then_some(index))
        .collect::<Vec<_>>();
    assert!(!sender_rows.is_empty());
    for sender_row in sender_rows {
        assert!(row_segment(&rows, sender_row, 33, 100).contains('▀'));
    }
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

#[test]
fn visible_chat_avatars_load_before_cursor_neighbors() -> Result<()> {
    let account = (0..20).fold(account("Ada").with_selected_chat(18), |account, offset| {
        account
            .with_chat(chat(10 + offset, format!("Chat {offset:02}")))
            .with_avatar(10 + offset)
    });
    let telegram = (16..20).fold(
        TelegramScenario::new().bootstrap(account),
        |scenario, peer| scenario.expect_avatar(peer),
    );
    let telegram = std::iter::once(19)
        .chain(22..30)
        .chain(10..15)
        .fold(telegram, |scenario, chat| {
            scenario.expect_load_history(chat, [])
        });
    let mut app = TestSystem::builder()
        .name("layout-visible-chat-avatar-priority")
        .terminal(100, 24)
        .telegram(telegram)
        .start()?;

    let rows = app.screen().rows();
    for offset in 6..10 {
        let row = row_within(&rows, &format!("Chat {offset:02}"), 0, 32);
        assert!(row_segment(&rows, row, 0, 32).contains('▀'));
    }
    app.expect_no_unhandled_work()
}
