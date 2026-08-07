use std::path::PathBuf;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::{RecoveryAction, RecoveryView, render, resolve_recovery_event};

#[test]
fn rebuild_key_is_unavailable_when_unique_records_are_unreadable() {
    let key = Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('b'),
        KeyModifiers::NONE,
        KeyEventKind::Press,
    ));

    assert_eq!(
        resolve_recovery_event(key.clone(), true),
        Some(RecoveryAction::RebuildCache)
    );
    assert_eq!(resolve_recovery_event(key, false), None);
}

#[test]
fn recovery_screen_shows_exact_database_and_backup_paths() {
    let view = RecoveryView {
        account_name: "Ada".to_owned(),
        database_path: PathBuf::from("/data/7.db"),
        backup_paths: vec![PathBuf::from("/data/7.db.pre-migration-1.bak")],
        failure: "foreign key check failed".to_owned(),
        rebuild_blocker: None,
        completion: None,
        notice: None,
    };
    let mut terminal =
        Terminal::new(TestBackend::new(100, 24)).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, &view))
        .expect("recovery view should render");
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert!(text.contains("/data/7.db"));
    assert!(text.contains("/data/7.db.pre-migration-1.bak"));
    assert!(text.contains("Rebuild Cache"));
}
