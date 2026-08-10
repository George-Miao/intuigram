use intuigram_lib::{AccountKey, AccountLifecycle};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, key};

#[test]
fn account_picker_lists_switches_and_starts_add_account() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("accounts-picker")
        .telegram(
            TelegramScenario::new().bootstrap(
                account("Ada")
                    .with_identity(10)
                    .with_registered_account(20, "Grace")
                    .with_chat(chat(10, "Rust")),
            ),
        )
        .start()?;

    app.press(key::ACCOUNTS)?;
    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains("Accounts")));
    assert!(rows.iter().any(|row| row.contains("Ada")));
    assert!(rows.iter().any(|row| row.contains("Grace")));
    assert!(rows.iter().any(|row| row.contains("Add Account")));
    app.press(key::DOWN)?;
    app.press(key::ENTER)?;
    app.expect_account_lifecycle(AccountLifecycle::Switch(AccountKey(20)))?;

    app.press(key::ACCOUNTS)?;
    app.press(key::DOWN)?;
    app.press(key::DOWN)?;
    app.press(key::ENTER)?;
    app.expect_account_lifecycle(AccountLifecycle::Add)?;
    app.expect_no_unhandled_work()
}

#[test]
fn logout_and_remove_locally_have_distinct_explicit_confirmations() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("accounts-confirmations")
        .telegram(
            TelegramScenario::new().bootstrap(
                account("Ada")
                    .with_identity(10)
                    .with_registered_account(20, "Grace")
                    .with_chat(chat(10, "Rust")),
            ),
        )
        .start()?;

    app.press(key::ACCOUNTS)?;
    app.press(key::ALT_LOGOUT)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Log out Ada (10)?"))
    );
    app.press(key::ENTER)?;
    app.expect_account_lifecycle(AccountLifecycle::Logout(AccountKey(10)))?;

    app.press(key::ACCOUNTS)?;
    app.press(key::DOWN)?;
    app.press(key::ALT_REMOVE_LOCAL)?;
    let rows = app.screen().rows();
    assert!(
        rows.iter()
            .any(|row| row.contains("Remove Grace (20) locally?"))
    );
    assert!(
        rows.iter()
            .any(|row| row.contains("Telegram authorization remains active"))
    );
    app.press(key::ENTER)?;
    app.expect_account_lifecycle(AccountLifecycle::RemoveLocal(AccountKey(20)))?;
    app.expect_no_unhandled_work()
}
