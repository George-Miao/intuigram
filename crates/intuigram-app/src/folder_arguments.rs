use crate::launch::{Error, InvalidArgumentValueSnafu, Result, UnknownArgumentSnafu};
use crate::{FolderMaintenance, FolderRules};

pub(super) fn parse_folder_maintenance(
    arguments: &mut impl Iterator<Item = String>,
    action: &str,
    label: &str,
) -> Result<FolderMaintenance> {
    match action {
        "create" => Ok(FolderMaintenance::Create {
            title: next_argument(arguments, label)?,
            rules: parse_folder_rules(label, next_argument(arguments, label)?)?,
        }),
        "rename" => Ok(FolderMaintenance::Rename {
            folder: parse_folder_id(label, next_argument(arguments, label)?)?,
            title: next_argument(arguments, label)?,
        }),
        "reorder" => Ok(FolderMaintenance::Reorder {
            folder: parse_folder_id(label, next_argument(arguments, label)?)?,
            position: parse_position(label, next_argument(arguments, label)?)?,
        }),
        "share" => Ok(FolderMaintenance::Share {
            folder: parse_folder_id(label, next_argument(arguments, label)?)?,
        }),
        "delete" => Ok(FolderMaintenance::Delete {
            folder: parse_folder_id(label, next_argument(arguments, label)?)?,
        }),
        "rules" => Ok(FolderMaintenance::Rules {
            folder: parse_folder_id(label, next_argument(arguments, label)?)?,
            rules: parse_folder_rules(label, next_argument(arguments, label)?)?,
        }),
        _ => UnknownArgumentSnafu {
            argument: label.to_owned(),
        }
        .fail(),
    }
}

pub(super) fn next_argument(
    arguments: &mut impl Iterator<Item = String>,
    argument: &str,
) -> Result<String> {
    arguments.next().ok_or_else(|| Error::MissingArgumentValue {
        argument: argument.to_owned(),
    })
}

fn parse_folder_id(argument: &str, value: String) -> Result<i32> {
    value
        .parse::<i32>()
        .ok()
        .filter(|folder| *folder > 1)
        .ok_or_else(|| Error::InvalidArgumentValue {
            argument: argument.to_owned(),
            value,
        })
}

fn parse_position(argument: &str, value: String) -> Result<usize> {
    value
        .parse::<usize>()
        .map_err(|_| Error::InvalidArgumentValue {
            argument: argument.to_owned(),
            value,
        })
}

fn parse_folder_rules(argument: &str, value: String) -> Result<FolderRules> {
    let mut rules = FolderRules::default();
    for rule in value.split(',').filter(|rule| !rule.is_empty()) {
        match rule {
            "contacts" => rules.contacts = true,
            "non-contacts" => rules.non_contacts = true,
            "groups" => rules.groups = true,
            "channels" => rules.broadcasts = true,
            "bots" => rules.bots = true,
            "exclude-muted" => rules.exclude_muted = true,
            "exclude-read" => rules.exclude_read = true,
            "exclude-archived" => rules.exclude_archived = true,
            _ => {
                return InvalidArgumentValueSnafu {
                    argument: argument.to_owned(),
                    value,
                }
                .fail();
            }
        }
    }
    Ok(rules)
}
