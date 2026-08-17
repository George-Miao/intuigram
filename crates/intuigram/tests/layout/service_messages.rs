use super::*;

#[test]
fn service_message_sender_and_event_are_centered() -> Result<()> {
    let mut service = incoming(41, "Mohit Medheshiya", "Added 1 member(s)");
    service.details.service = Some("Added 1 member(s)".to_owned());
    let mut app = TestSystem::builder()
        .name("layout-centered-service-message")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [service]),
        )
        .start()?;

    app.press(key::ENTER)?;
    let screen = app.screen();
    screen
        .message(41)
        .expect_text_centered("Mohit Medheshiya")?;
    screen
        .message(41)
        .expect_text_centered("Added 1 member(s)")?;
    app.press(key::ALT_UP)?;
    screen.message(41).expect_active()?;
    screen
        .message(41)
        .expect_text_centered("Mohit Medheshiya")?;
    screen
        .message(41)
        .expect_text_centered("Added 1 member(s)")?;
    app.expect_no_unhandled_work()
}
