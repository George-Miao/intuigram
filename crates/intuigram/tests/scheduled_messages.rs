use test_harness::{Result, TelegramScenario, TestSystem, account, chat, key};

fn scheduled(name: &str) -> Result<TestSystem> {
    let mut app = TestSystem::builder()
        .name(name)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, []),
        )
        .start()?;
    app.press(key::ENTER)?;
    app.choose_action("Scheduled Messages")?;
    Ok(app)
}

fn erase(app: &mut TestSystem, characters: usize) -> Result<()> {
    for _ in 0..characters {
        app.press(key::BACKSPACE)?;
    }
    Ok(())
}

fn create(app: &mut TestSystem, text: &str, delivery: &str) -> Result<()> {
    app.press(key::NEW_SCHEDULED)?;
    app.type_text(text)?;
    app.press(key::DOWN)?;
    app.type_text(delivery)?;
    app.press(key::ENTER)
}

#[test]
fn scheduled_history_creates_edits_reschedules_and_sends_now() -> Result<()> {
    let mut app = scheduled("scheduled-lifecycle")?;

    create(&mut app, "initial", "2030-06-01T09:30:00+08:00")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("initial"))
    );
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("2030-06-01T01:30:00Z"))
    );

    app.press(key::EDIT_SCHEDULED)?;
    erase(&mut app, "initial".len())?;
    app.type_text("updated")?;
    app.press(key::ENTER)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("updated"))
    );

    app.press(key::RESCHEDULE)?;
    erase(&mut app, "2030-06-01T01:30:00Z".len())?;
    app.type_text("online")?;
    app.press(key::ENTER)?;
    assert!(app.screen().rows().iter().any(|row| row.contains("online")));

    app.press(key::SEND_NOW)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("deliver it immediately"))
    );
    app.press(key::ENTER)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("No Scheduled Messages"))
    );
    app.press(key::ESCAPE)?;
    assert!(
        !app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("updated"))
    );
    app.expect_no_unhandled_work()
}

#[test]
fn scheduled_delete_requires_confirmation_and_does_not_send() -> Result<()> {
    let mut app = scheduled("scheduled-delete")?;
    create(&mut app, "discard me", "online")?;

    app.press(key::DELETE_SCHEDULED)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("removed without being sent"))
    );
    app.press(key::ENTER)?;

    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("No Scheduled Messages"))
    );
    app.expect_no_unhandled_work()
}
