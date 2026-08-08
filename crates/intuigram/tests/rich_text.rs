use intuigram_app::{DeliveryState, TextEntity, TextEntityKind};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, key, sent_message};

#[test]
fn markdown_style_composer_text_is_sent_as_multiline_telegram_entities() -> Result<()> {
    let entities = vec![
        TextEntity {
            offset: 0,
            length: 5,
            kind: TextEntityKind::Bold,
        },
        TextEntity {
            offset: 6,
            length: 5,
            kind: TextEntityKind::Code,
        },
    ];
    let mut app = TestSystem::builder()
        .name("rich-text-send")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [])
                .hold_send_rich_text("rich", 10, "Hello\nworld", entities),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.type_text("**Hello**")?;
    app.press(key::SHIFT_ENTER)?;
    app.type_text("`world`")?;
    app.press(key::ENTER)?;

    app.screen()
        .message_text("Hello\nworld")
        .expect_delivery(DeliveryState::Pending)?;
    app.telegram()
        .complete("rich", sent_message(41, "Hello\nworld"))?;
    app.expect_no_unhandled_work()
}

#[test]
fn editing_with_markup_replaces_text_and_entities_together() -> Result<()> {
    let entity = TextEntity {
        offset: 0,
        length: 5,
        kind: TextEntityKind::Bold,
    };
    let mut updated = sent_message(40, "Hello");
    updated.details.entities = vec![entity.clone()];
    let mut app = TestSystem::builder()
        .name("rich-text-edit")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [sent_message(40, "old")])
                .expect_rich_edit_message(10, 40, "Hello", vec![entity], updated),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::UP)?;
    for _ in 0..3 {
        app.press(key::BACKSPACE)?;
    }
    app.type_text("**Hello**")?;
    app.press(key::ENTER)?;

    app.screen()
        .message_text("Hello")
        .expect_delivery(DeliveryState::Sent)?;
    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("")?;
    app.expect_no_unhandled_work()
}
