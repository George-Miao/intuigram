use test_harness::{Result, TelegramScenario, TestSystem, account, chat, key};

#[test]
fn custom_folder_can_be_created_with_category_rules() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("folder-create")
        .telegram(TelegramScenario::new().bootstrap(account("Ada").with_chat(chat(10, "Rust"))))
        .start()?;

    app.press(key::FOLDER_SETTINGS)?;
    app.press(key::NEW_FOLDER)?;
    app.type_text("People")?;
    app.press(key::DOWN)?;
    app.press(key::SPACE)?;
    app.press(key::ENTER)?;

    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains("People")));
    app.press(key::EDIT_FOLDER)?;
    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains("[x] Contacts")));
    app.expect_no_unhandled_work()
}

#[test]
fn custom_folder_can_be_renamed_reordered_shared_and_deleted() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("folder-lifecycle")
        .telegram(
            TelegramScenario::new().bootstrap(
                account("Ada")
                    .with_folder(2, "Work")
                    .with_folder(3, "Friends")
                    .with_chat(chat(10, "Rust")),
            ),
        )
        .start()?;

    app.press(key::FOLDER_SETTINGS)?;
    app.press(key::SHIFT_DOWN)?;
    let rows = app.screen().rows();
    let friends = rows
        .iter()
        .position(|row| row.contains("Friends"))
        .expect("Friends remains visible");
    let work = rows
        .iter()
        .position(|row| row.contains("Work"))
        .expect("Work remains visible");
    assert!(friends < work);

    app.press(key::EDIT_FOLDER)?;
    app.type_text(" Updated")?;
    app.press(key::ENTER)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Work Updated"))
    );

    app.press(key::SHARE_FOLDER)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("https://t.me/addlist/folder-2"))
    );

    app.press(key::DELETE_FOLDER)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Delete Work Updated?"))
    );
    app.press(key::ENTER)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .all(|row| !row.contains("Work Updated"))
    );
    app.expect_no_unhandled_work()
}
