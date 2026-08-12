use intuigram_lib::DeliveryState;
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, key, sent_message};

#[test]
fn ordinary_url_messages_request_a_telegram_link_preview() -> Result<()> {
    let url = "https://example.com/intuigram";
    let mut app = TestSystem::builder()
        .name("link-preview-send")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [])
                .hold_send_with_link_preview("url", 10, url),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.type_text(url)?;
    app.press(key::ENTER)?;

    app.screen()
        .message_text(url)
        .expect_delivery(DeliveryState::Pending)?;
    app.telegram().complete("url", sent_message(41, url))?;
    app.screen()
        .message_text(url)
        .expect_delivery(DeliveryState::Sent)?;
    app.expect_no_unhandled_work()
}
