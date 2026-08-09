use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::render::render_login;
use super::*;

#[test]
fn login_form_is_centered_stepped_and_masks_secrets() {
    let mut terminal =
        Terminal::new(TestBackend::new(100, 30)).expect("terminal should initialize");
    let prompt = LoginPrompt {
        field: LoginField::ApplicationHash,
        label: "Application hash",
        description: "Create an application at my.telegram.org/apps.",
        error: Some("Hash is required"),
        secret: true,
        can_go_back: true,
    };
    terminal
        .draw(|frame| render_login(frame, &prompt, "secret"))
        .expect("login form should render");
    let buffer = terminal.backend().buffer();
    let rendered = buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert!(rendered.contains("Connect Intuigram"));
    assert!(rendered.contains("Step 2 of 5"));
    assert!(rendered.contains("••••••"));
    assert!(!rendered.contains("secret"));
    assert!(rendered.contains("Shift+Tab Back"));
    assert!(rendered.contains("Hash is required"));
}
