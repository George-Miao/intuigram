use super::*;

impl App {
    pub(super) fn open_account_picker(&mut self) {
        self.view.account_picker = Some(
            self.view
                .accounts
                .iter()
                .position(|account| account.active)
                .unwrap_or(0),
        );
    }

    pub(super) fn apply_account_picker_action(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::MoveUp | Action::MoveDown => {
                let entries = self.view.accounts.len().saturating_add(1);
                let current = self.view.account_picker.unwrap_or(0);
                self.view.account_picker =
                    move_index(Some(current), entries, action == Action::MoveDown);
                None
            }
            Action::ConfirmAccount => {
                let selected = self.view.account_picker.take()?;
                let request = self
                    .view
                    .accounts
                    .get(selected)
                    .filter(|account| !account.active)
                    .map_or(AccountLifecycle::Add, |account| {
                        AccountLifecycle::Switch(account.id)
                    });
                if self
                    .view
                    .accounts
                    .get(selected)
                    .is_some_and(|account| account.active)
                {
                    return None;
                }
                Some(Effect::AccountLifecycle { request })
            }
            Action::LogoutAccount => {
                let account = self.selected_account().filter(|account| account.active)?;
                self.view.account_confirmation = Some(AccountConfirmationView {
                    account: account.id,
                    kind: AccountConfirmationKind::Logout,
                });
                self.view.account_picker = None;
                None
            }
            Action::RemoveAccountLocally => {
                let account = self.selected_account()?;
                self.view.account_confirmation = Some(AccountConfirmationView {
                    account: account.id,
                    kind: AccountConfirmationKind::RemoveLocal,
                });
                self.view.account_picker = None;
                None
            }
            Action::Cancel | Action::ManageAccounts => {
                self.view.account_picker = None;
                None
            }
            _ => None,
        }
    }

    pub(super) fn apply_account_confirmation(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::ConfirmAccountOperation => {
                let confirmation = self.view.account_confirmation.take()?;
                let request = match confirmation.kind {
                    AccountConfirmationKind::Logout => {
                        AccountLifecycle::Logout(confirmation.account)
                    }
                    AccountConfirmationKind::RemoveLocal => {
                        AccountLifecycle::RemoveLocal(confirmation.account)
                    }
                };
                Some(Effect::AccountLifecycle { request })
            }
            Action::Cancel => {
                self.view.account_confirmation = None;
                None
            }
            _ => None,
        }
    }

    fn selected_account(&self) -> Option<&AccountView> {
        let selected = self.view.account_picker?;
        self.view.accounts.get(selected)
    }
}
