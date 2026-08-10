use intuigram_lib::AccountConfirmationKind;

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
                format!(
                    "  {}{}",
                    account.id.0,
                    if account.active { "  current" } else { "" }
                ),
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
    let (account, account_id) = view
        .accounts
        .iter()
        .find(|account| account.id == confirmation.account)
        .map_or(("Account", confirmation.account.0), |account| {
            (account.display_name.as_str(), account.id.0)
        });
    let (title, warning, consequence) = match confirmation.kind {
        AccountConfirmationKind::Logout => (
            format!("Log out {account} ({account_id})?"),
            "This revokes Telegram authorization.",
            "Deletes: local session, Local Records, and Media Cache.",
        ),
        AccountConfirmationKind::RemoveLocal => (
            format!("Remove {account} ({account_id}) locally?"),
            "Deletes: local session, Local Records, and Media Cache.",
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
            Line::from(Span::styled(consequence, Style::default().fg(MUTED_TEXT))),
        ],
    );
}
