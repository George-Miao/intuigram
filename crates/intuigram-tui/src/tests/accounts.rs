use super::*;

#[test]
fn picker_disambiguates_names_and_confirmation_names_deleted_data() {
    let mut current = view(Vec::new());
    current.accounts = vec![
        intuigram_app::AccountView {
            id: intuigram_app::AccountKey(10),
            display_name: "Ada".to_owned(),
            active: true,
        },
        intuigram_app::AccountView {
            id: intuigram_app::AccountKey(20),
            display_name: "Ada".to_owned(),
            active: false,
        },
    ];
    current.account_picker = Some(0);
    let backend = TestBackend::new(100, 28);
    let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, &current, &EffectiveKeymap::defaults()))
        .expect("Account picker should render");
    let picker = rendered_symbols(&terminal);
    assert!(picker.contains("Ada  10"));
    assert!(picker.contains("Ada  20"));

    current.account_picker = None;
    current.account_confirmation = Some(intuigram_app::AccountConfirmationView {
        account: intuigram_app::AccountKey(20),
        kind: intuigram_app::AccountConfirmationKind::RemoveLocal,
    });
    terminal
        .draw(|frame| render(frame, &current, &EffectiveKeymap::defaults()))
        .expect("Account confirmation should render");
    let confirmation = rendered_symbols(&terminal);
    assert!(confirmation.contains("Ada (20)"));
    assert!(confirmation.contains("local session"));
    assert!(confirmation.contains("Local Records"));
    assert!(confirmation.contains("Media Cache"));
}

fn rendered_symbols(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}
