use intuigram_app::AccountConfirmationKind;

use super::*;

pub(super) const ACCOUNT_BINDINGS: &[Binding] = &[
    binding(
        KeyChord::plain(Key::Char('a')),
        "Accounts",
        Action::ManageAccounts,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Choose Account",
        Action::ConfirmAccount,
        true,
    ),
    binding(
        KeyChord::alt(Key::Char('l')),
        "Log Out",
        Action::LogoutAccount,
        true,
    ),
    binding(
        KeyChord::alt(Key::Char('d')),
        "Remove Locally",
        Action::RemoveAccountLocally,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Confirm Account Operation",
        Action::ConfirmAccountOperation,
        true,
    ),
];

pub(super) fn render_account_picker(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let Some(selected) = view.account_picker else {
        return;
    };
    let popup = centered_rect(46, 52, area);
    let lines = std::iter::once(Line::from(Span::styled(
        "Accounts",
        Style::default().add_modifier(Modifier::BOLD),
    )))
    .chain(std::iter::once(Line::from("")))
    .chain(view.accounts.iter().enumerate().map(|(index, account)| {
        Line::from(vec![
            selection_rule(index == selected),
            Span::raw(account.display_name.clone()),
            Span::styled(
                if account.active { "  current" } else { "" },
                Style::default().fg(MUTED_TEXT),
            ),
        ])
    }))
    .chain(std::iter::once(Line::from(vec![
        selection_rule(selected == view.accounts.len()),
        Span::styled("+ Add Account", Style::default().fg(PRIMARY)),
    ])))
    .collect();
    render_overlays::render_overlay(frame, popup, lines);
}

pub(super) fn render_account_confirmation(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let Some(confirmation) = view.account_confirmation else {
        return;
    };
    let account = view
        .accounts
        .iter()
        .find(|account| account.id == confirmation.account)
        .map_or("Account", |account| account.display_name.as_str());
    let (title, warning) = match confirmation.kind {
        AccountConfirmationKind::Logout => (
            format!("Log out {account}?"),
            "Telegram authorization will be revoked before local data is removed.",
        ),
        AccountConfirmationKind::RemoveLocal => (
            format!("Remove {account} locally?"),
            "Telegram authorization remains active on Telegram's servers.",
        ),
    };
    let popup = centered_rect(62, 32, area);
    render_overlays::render_overlay(
        frame,
        popup,
        vec![
            Line::from(Span::styled(
                title,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(warning, Style::default().fg(MUTED_TEXT))),
        ],
    );
}
