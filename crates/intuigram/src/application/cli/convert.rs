use std::iter;

use clap::{CommandFactory, Parser};
use intuigram_store::AccountId;
use snafu::ResultExt;

use super::definition::Cli;
use crate::application::{
    Arguments, Error, Maintenance, ParseArgumentsSnafu, Result, parse_folder_maintenance,
    parse_media_maintenance, parse_scheduled_maintenance,
};

pub(in crate::application) fn parse_arguments(
    arguments: impl IntoIterator<Item = String>,
) -> Result<Arguments> {
    let mut cli = Cli::try_parse_from(iter::once("intuigram".to_owned()).chain(arguments))
        .context(ParseArgumentsSnafu)?;
    let account = cli
        .account
        .take()
        .map(|value| account("--account", value))
        .transpose()?;
    let maintenance = maintenance(&mut cli)?;
    Ok(Arguments {
        config: cli.config_dir,
        data: cli.data_dir,
        cache: cli.cache_dir,
        downloads: cli.downloads_dir,
        maintenance,
        account,
        add_account: cli.add_account,
        list_accounts: cli.list_accounts,
        test_connection: cli.test_connection,
        help: cli.help,
    })
}

pub(in crate::application) fn help_text() -> String {
    Cli::command().render_long_help().to_string()
}

pub(in crate::application) fn print_help() {
    print!("{}", help_text());
}

fn maintenance(cli: &mut Cli) -> Result<Option<Maintenance>> {
    if let Some(value) = cli.media_cache_usage.take() {
        return Ok(Some(Maintenance::MediaUsage(account(
            "--media-cache-usage",
            value,
        )?)));
    }
    if let Some(value) = cli.clear_media_cache.take() {
        return Ok(Some(Maintenance::ClearMedia(account(
            "--clear-media-cache",
            value,
        )?)));
    }
    if let Some(value) = cli.clear_account_data.take() {
        return Ok(Some(Maintenance::ClearAccount(account(
            "--clear-account-data",
            value,
        )?)));
    }
    if let Some(value) = cli.remove_account.take() {
        return Ok(Some(Maintenance::ClearAccount(account(
            "--remove-account",
            value,
        )?)));
    }
    if let Some(value) = cli.logout.take() {
        return Ok(Some(Maintenance::Logout(account("--logout", value)?)));
    }
    if let Some(maintenance) = folder(cli)? {
        return Ok(Some(maintenance));
    }
    if let Some(maintenance) = rich_media(cli)? {
        return Ok(Some(maintenance));
    }
    scheduled(cli)
}

fn folder(cli: &mut Cli) -> Result<Option<Maintenance>> {
    for (argument, values) in [
        ("--folder-create", cli.folder_create.take()),
        ("--folder-rename", cli.folder_rename.take()),
        ("--folder-reorder", cli.folder_reorder.take()),
        ("--folder-share", cli.folder_share.take()),
        ("--folder-delete", cli.folder_delete.take()),
        ("--folder-rules", cli.folder_rules.take()),
    ] {
        if let Some(values) = values {
            return command(
                argument,
                values,
                parse_folder_maintenance,
                Maintenance::Folder,
            )
            .map(Some);
        }
    }
    Ok(None)
}

fn rich_media(cli: &mut Cli) -> Result<Option<Maintenance>> {
    for (argument, values) in [
        ("--media-browse", cli.media_browse.take()),
        ("--media-send", cli.media_send.take()),
        ("--media-file", cli.media_file.take()),
        ("--record-media", cli.record_media.take()),
        ("--send-contact", cli.send_contact.take()),
    ] {
        if let Some(values) = values {
            return command(
                argument,
                values,
                parse_media_maintenance,
                Maintenance::RichMedia,
            )
            .map(Some);
        }
    }
    Ok(None)
}

fn scheduled(cli: &mut Cli) -> Result<Option<Maintenance>> {
    for (argument, values) in [
        ("--schedule-message", cli.schedule_message.take()),
        ("--scheduled-list", cli.scheduled_list.take()),
        ("--scheduled-edit", cli.scheduled_edit.take()),
        ("--scheduled-reschedule", cli.scheduled_reschedule.take()),
        ("--scheduled-delete", cli.scheduled_delete.take()),
        ("--scheduled-send-now", cli.scheduled_send_now.take()),
    ] {
        if let Some(values) = values {
            return command(
                argument,
                values,
                parse_scheduled_maintenance,
                Maintenance::Scheduled,
            )
            .map(Some);
        }
    }
    Ok(None)
}

fn command<T>(
    argument: &str,
    values: Vec<String>,
    parse: impl FnOnce(&mut std::vec::IntoIter<String>, &str) -> Result<T>,
    wrap: impl FnOnce(AccountId, T) -> Maintenance,
) -> Result<Maintenance> {
    let mut values = values.into_iter();
    let account_value = values.next().ok_or_else(|| Error::MissingArgumentValue {
        argument: argument.to_owned(),
    })?;
    let account = account(argument, account_value)?;
    parse(&mut values, argument).map(|command| wrap(account, command))
}

fn account(argument: &str, value: String) -> Result<AccountId> {
    value
        .parse::<i64>()
        .ok()
        .and_then(AccountId::new)
        .ok_or_else(|| Error::InvalidArgumentValue {
            argument: argument.to_owned(),
            value,
        })
}
