use intuigram_app::{AdapterEvent, ChatId, MessageId};
use intuigram_telegram::{UpdateCursor, UpdateScope};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn unread_divider_survives_live_updates_and_clears_when_read_state_advances() -> Result<()> {
    let mut room = chat(10, "Rust");
    room.unread = 2;
    let mut app = TestSystem::builder()
        .name("unread-divider")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(room))
                .expect_load_history(
                    10,
                    [
                        incoming(40, "Lin", "read"),
                        incoming(41, "Lin", "first unread"),
                        incoming(42, "Lin", "second unread"),
                    ],
                ),
        )
        .start()?;

    app.press(key::ENTER)?;
    expect_divider_before(&app, "first unread");
    app.telegram().inject_update(
        cursor(1),
        AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(incoming(43, "Lin", "live unread")),
        },
    )?;
    expect_divider_before(&app, "first unread");

    app.telegram().inject_update(
        cursor(2),
        AdapterEvent::HistoryRead {
            chat: ChatId(10),
            max_id: MessageId(42),
            outgoing: false,
            unread: Some(1),
        },
    )?;
    expect_divider_before(&app, "live unread");

    app.telegram().inject_update(
        cursor(3),
        AdapterEvent::HistoryRead {
            chat: ChatId(10),
            max_id: MessageId(43),
            outgoing: false,
            unread: Some(0),
        },
    )?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .all(|row| !row.contains("Unread messages"))
    );
    app.expect_no_unhandled_work()
}

fn expect_divider_before(app: &TestSystem, body: &str) {
    let rows = app.screen().rows();
    let divider = rows
        .iter()
        .position(|row| row.contains("Unread messages"))
        .expect("Unread divider should be visible");
    let message = rows
        .iter()
        .enumerate()
        .skip(divider + 1)
        .find(|(_, row)| row.contains(body))
        .map(|(index, _)| index)
        .expect("boundary Message should be visible");
    assert_eq!(divider + 2, message);
}

fn cursor(pts: i32) -> UpdateCursor {
    UpdateCursor {
        scope: UpdateScope::Account,
        pts: Some(pts),
        pts_count: 1,
        ..UpdateCursor::default()
    }
}
