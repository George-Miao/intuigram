use intuigram_app::{MediaCard, MediaKind, MessageDetails, PollOptionView, PollView};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn poll_composer_sends_a_question_and_two_or_more_options() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("poll-composer")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [])
                .expect_send_poll(10, "Best language?", ["Rust", "Zig"]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.choose_action("Create Poll")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Poll · question first, then one option per line"))
    );
    app.type_text("Best language?")?;
    app.press(key::SHIFT_ENTER)?;
    app.type_text("Rust")?;
    app.press(key::SHIFT_ENTER)?;
    app.type_text("Zig")?;
    app.press(key::ENTER)?;

    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("[Poll]  Best language?"))
    );
    app.expect_no_unhandled_work()
}

#[test]
fn open_multiple_choice_poll_can_be_voted_from_the_transcript() -> Result<()> {
    let poll = poll_message(false);
    let updated = poll_message(true);
    let mut app = TestSystem::builder()
        .name("poll-vote")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [poll])
                .expect_vote_poll(10, 40, [0, 1], updated),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Vote")?;
    app.press(key::SPACE)?;
    app.press(key::DOWN)?;
    app.press(key::SPACE)?;
    app.press(key::ENTER)?;

    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains("● Rust · 3")));
    assert!(rows.iter().any(|row| row.contains("● Zig · 2")));
    app.expect_no_unhandled_work()
}

fn poll_message(voted: bool) -> intuigram_app::MessageView {
    let mut message = incoming(40, "Lin", "");
    message.details = MessageDetails {
        media: Some(MediaCard {
            kind: MediaKind::Poll,
            title: "Poll".to_owned(),
            description: "Best language?".to_owned(),
            details: Vec::new(),
            poll: Some(PollView {
                quiz: false,
                multiple_choice: true,
                closed: false,
                total_voters: Some(if voted { 5 } else { 3 }),
                options: vec![
                    PollOptionView {
                        text: "Rust".to_owned(),
                        voters: Some(3),
                        chosen: voted,
                        correct: false,
                    },
                    PollOptionView {
                        text: "Zig".to_owned(),
                        voters: Some(if voted { 2 } else { 0 }),
                        chosen: voted,
                        correct: false,
                    },
                ],
                solution: None,
            }),
            remote_id: Some("77".to_owned()),
        }),
        ..MessageDetails::default()
    };
    message
}
