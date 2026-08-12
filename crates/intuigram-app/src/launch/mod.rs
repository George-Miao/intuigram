//! Process-facing launch arguments without command-line framework types.

mod folder;
mod media;
mod scheduled;

use std::path::PathBuf;

use intuigram_store::AccountId;
use snafu::Snafu;

use super::Maintenance;

/// Platform-directory overrides supplied by the process entrypoint.
#[derive(Debug, Default)]
pub struct Directories {
    /// Overrides the platform configuration directory.
    pub config: Option<PathBuf>,

    /// Overrides the platform data directory.
    pub data: Option<PathBuf>,

    /// Overrides the platform cache directory.
    pub cache: Option<PathBuf>,

    /// Overrides the platform Downloads directory.
    pub downloads: Option<PathBuf>,
}
/// One validated application launch request.
pub struct Arguments {
    pub(super) global: Global,
    pub(super) command: Command,
}

/// Validated process-global launch flags.
pub struct Global {
    pub(super) config: Option<PathBuf>,
    pub(super) data: Option<PathBuf>,
    pub(super) cache: Option<PathBuf>,
    pub(super) downloads: Option<PathBuf>,
    pub(super) account: Option<AccountId>,
    pub(super) test_connection: bool,
}

/// An application command selected by the process entrypoint.
#[derive(Clone, Default)]
pub enum Command {
    /// Starts the terminal interface.
    #[default]
    Start,

    /// Authorizes and registers another Telegram Account.
    AccountAdd,

    /// Lists registered Telegram Accounts without opening the TUI.
    AccountList,

    /// Runs one Account-scoped maintenance operation.
    Maintenance(MaintenanceCommand),
}

/// One validated Account-scoped maintenance operation.
#[derive(Clone)]
pub struct MaintenanceCommand(Maintenance);

impl Command {
    /// Starts the terminal interface.
    #[must_use]
    pub const fn start() -> Self {
        Self::Start
    }

    /// Authorizes and registers another Telegram Account.
    #[must_use]
    pub const fn account_add() -> Self {
        Self::AccountAdd
    }

    /// Lists registered Telegram Accounts without opening the TUI.
    #[must_use]
    pub const fn account_list() -> Self {
        Self::AccountList
    }

    /// Clears all local data for the selected Account.
    #[must_use]
    pub const fn account_clear_data() -> Self {
        Self::maintenance(Maintenance::ClearAccount)
    }

    /// Removes the selected Account's local data.
    #[must_use]
    pub const fn account_remove() -> Self {
        Self::maintenance(Maintenance::ClearAccount)
    }

    /// Revokes authorization and removes the selected Account.
    #[must_use]
    pub const fn account_logout() -> Self {
        Self::maintenance(Maintenance::Logout)
    }

    /// Reports media-cache usage for the selected Account.
    #[must_use]
    pub const fn cache_usage() -> Self {
        Self::maintenance(Maintenance::MediaUsage)
    }

    /// Clears redownloadable media for the selected Account.
    #[must_use]
    pub const fn cache_clear() -> Self {
        Self::maintenance(Maintenance::ClearMedia)
    }

    const fn maintenance(maintenance: Maintenance) -> Self {
        Self::Maintenance(MaintenanceCommand(maintenance))
    }
}

impl Global {
    /// Validates process-global flags.
    pub fn new(
        directories: Directories,
        account: Option<String>,
        test_connection: bool,
    ) -> Result<Self> {
        let account = account
            .map(|value| parse_account("--account", value))
            .transpose()?;
        Ok(Self {
            config: directories.config,
            data: directories.data,
            cache: directories.cache,
            downloads: directories.downloads,
            account,
            test_connection,
        })
    }
}

impl Arguments {
    /// Validates one command against its process-global flags.
    pub fn new(global: Global, command: Command) -> Result<Self> {
        if matches!(&command, Command::AccountAdd) && global.account.is_some() {
            return ConflictingAccountSelectionSnafu.fail();
        }
        if global.test_connection && !matches!(&command, Command::Start) {
            return ConflictingConnectionTestSnafu.fail();
        }
        if matches!(&command, Command::Maintenance(_)) && global.account.is_none() {
            return MissingAccountSnafu.fail();
        }
        Ok(Self { global, command })
    }
}

impl MaintenanceCommand {
    pub(super) fn into_inner(self) -> Maintenance {
        self.0
    }
}

/// Launch-argument validation failure.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    /// A command omitted a required positional value.
    #[snafu(display("missing value for {argument}"))]
    MissingArgumentValue { argument: String },

    /// A command supplied a value outside its accepted domain.
    #[snafu(display("{argument} has invalid value {value:?}"))]
    InvalidArgumentValue { argument: String, value: String },

    /// An internal command conversion named an unsupported operation.
    #[snafu(display("unknown command {argument}"))]
    UnknownArgument { argument: String },

    /// An Account-scoped command omitted the global Account selector.
    #[snafu(display("this command requires --account <ID>"))]
    MissingAccount,

    /// Account addition also selected an existing Account.
    #[snafu(display("--account cannot be used with `account add`"))]
    ConflictingAccountSelection,

    /// A connection-only probe was combined with a stateful command.
    #[snafu(display("--test-connection cannot be combined with a stateful command"))]
    ConflictingConnectionTest,
}

/// Result of validating process-owned launch arguments.
pub type Result<T, E = Error> = std::result::Result<T, E>;

fn parse_account(argument: &str, value: String) -> Result<AccountId> {
    value
        .parse::<i64>()
        .ok()
        .and_then(AccountId::new)
        .ok_or_else(|| Error::InvalidArgumentValue {
            argument: argument.to_owned(),
            value,
        })
}
