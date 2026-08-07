/// Stable Telegram user identifier for one registered Account.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AccountKey(pub i64);

/// One Account available from the in-application Account picker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountView {
    /// Telegram user identity.
    pub id: AccountKey,

    /// User-facing Account name.
    pub display_name: String,

    /// Whether this Account owns the currently rendered state.
    pub active: bool,
}

/// Lifecycle transition requested from the composition root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountLifecycle {
    /// Replace the active state owner with another registered Account.
    Switch(AccountKey),

    /// Start a new Account authorization flow.
    Add,

    /// Revoke Telegram authorization, then remove local Account data.
    Logout(AccountKey),

    /// Remove local Account data without claiming Telegram revocation.
    RemoveLocal(AccountKey),
}

/// Destructive Account operation awaiting explicit confirmation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountConfirmationKind {
    /// Revoke the Telegram authorization before local removal.
    Logout,

    /// Remove only local state and leave Telegram authorization active.
    RemoveLocal,
}

/// Account confirmation overlay state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountConfirmationView {
    /// Target Account.
    pub account: AccountKey,

    /// Destructive operation being confirmed.
    pub kind: AccountConfirmationKind,
}
