#[cfg(test)]
use clap::Parser;
use clap::error::ErrorKind;
use clap::{ColorChoice, CommandFactory, FromArgMatches};
#[cfg(test)]
use intuigram_app::ArgumentError;
use intuigram_app::{ArgumentResult, Arguments, Command as LaunchCommand, Directories, Global};

use super::definition::{
    AccountCommand, CacheCommand, Cli, Command, FolderCommand, MediaCommand, ScheduledCommand,
};

pub(crate) fn parse() -> Arguments {
    let matches = command_factory().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
    convert(cli).unwrap_or_else(|error| {
        command_factory()
            .error(ErrorKind::ValueValidation, error)
            .exit()
    })
}

fn command_factory() -> clap::Command {
    let color = if std::env::var_os("CLICOLOR_FORCE").is_some() {
        ColorChoice::Always
    } else {
        ColorChoice::Auto
    };
    Cli::command().color(color)
}

fn convert(cli: Cli) -> ArgumentResult<Arguments> {
    let command = command(cli.command)?;
    let global = Global::new(
        Directories {
            config: cli.config_dir,
            data: cli.data_dir,
            cache: cli.cache_dir,
            downloads: cli.downloads_dir,
        },
        cli.account,
        cli.test_connection,
    )?;
    Arguments::new(global, command)
}

fn command(command: Option<Command>) -> ArgumentResult<LaunchCommand> {
    match command {
        None | Some(Command::Start) => Ok(LaunchCommand::start()),
        Some(Command::Account { command }) => Ok(account(command)),
        Some(Command::Cache { command }) => Ok(cache(command)),
        Some(Command::Folder { command }) => folder(command),
        Some(Command::Media { command }) => media(command),
        Some(Command::Scheduled { command }) => scheduled(command),
    }
}

fn account(command: AccountCommand) -> LaunchCommand {
    match command {
        AccountCommand::Add => LaunchCommand::account_add(),
        AccountCommand::List => LaunchCommand::account_list(),
        AccountCommand::ClearData => LaunchCommand::account_clear_data(),
        AccountCommand::Remove => LaunchCommand::account_remove(),
        AccountCommand::Logout => LaunchCommand::account_logout(),
    }
}

fn cache(command: CacheCommand) -> LaunchCommand {
    match command {
        CacheCommand::Usage => LaunchCommand::cache_usage(),
        CacheCommand::Clear => LaunchCommand::cache_clear(),
    }
}

fn folder(command: FolderCommand) -> ArgumentResult<LaunchCommand> {
    match command {
        FolderCommand::Create { title, rules } => LaunchCommand::folder_create(title, rules),
        FolderCommand::Rename { folder, title } => LaunchCommand::folder_rename(folder, title),
        FolderCommand::Reorder { folder, position } => {
            LaunchCommand::folder_reorder(folder, position)
        }
        FolderCommand::Share { folder } => LaunchCommand::folder_share(folder),
        FolderCommand::Delete { folder } => LaunchCommand::folder_delete(folder),
        FolderCommand::Rules { folder, rules } => LaunchCommand::folder_rules(folder, rules),
    }
}

fn media(command: MediaCommand) -> ArgumentResult<LaunchCommand> {
    match command {
        MediaCommand::Browse { kind, query } => LaunchCommand::media_browse(kind, query),
        MediaCommand::Send {
            chat,
            kind,
            index,
            query,
        } => LaunchCommand::media_send(chat, kind, index, query),
        MediaCommand::File { chat, kind, path } => LaunchCommand::media_file(chat, kind, path),
        MediaCommand::Record {
            chat,
            kind,
            seconds,
            device,
        } => LaunchCommand::media_record(chat, kind, seconds, device),
        MediaCommand::Contact {
            chat,
            phone,
            first_name,
            last_name,
        } => LaunchCommand::media_contact(chat, phone, first_name, last_name),
    }
}

fn scheduled(command: ScheduledCommand) -> ArgumentResult<LaunchCommand> {
    match command {
        ScheduledCommand::Create {
            chat,
            delivery,
            text,
        } => LaunchCommand::scheduled_create(chat, delivery, text),
        ScheduledCommand::List { chat } => LaunchCommand::scheduled_list(chat),
        ScheduledCommand::Edit {
            chat,
            message,
            text,
        } => LaunchCommand::scheduled_edit(chat, message, text),
        ScheduledCommand::Reschedule {
            chat,
            message,
            delivery,
        } => LaunchCommand::scheduled_reschedule(chat, message, delivery),
        ScheduledCommand::Delete { chat, message } => {
            LaunchCommand::scheduled_delete(chat, message)
        }
        ScheduledCommand::SendNow { chat, message } => {
            LaunchCommand::scheduled_send_now(chat, message)
        }
    }
}

#[cfg(test)]
fn parse_from(arguments: impl IntoIterator<Item = String>) -> Result<Arguments, clap::Error> {
    let cli = Cli::try_parse_from(std::iter::once("intuigram".to_owned()).chain(arguments))?;
    convert(cli)
        .map_err(|error: ArgumentError| Cli::command().error(ErrorKind::ValueValidation, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_no_command_starts_tui() {
        parse_from([]).expect("the default launch should start the TUI");
    }

    #[test]
    fn parse_account_command_requires_selector() {
        assert!(parse_from(["cache".to_owned(), "usage".to_owned()]).is_err());
        parse_from([
            "cache".to_owned(),
            "usage".to_owned(),
            "--account".to_owned(),
            "42".to_owned(),
        ])
        .expect("the global Account selector should apply after nested commands");
    }
}
