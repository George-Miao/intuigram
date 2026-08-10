use super::*;

pub(super) fn parse_folder_maintenance(
    arguments: &mut impl Iterator<Item = String>,
    argument: &str,
) -> Result<FolderMaintenance> {
    match argument {
        "--folder-create" => Ok(FolderMaintenance::Create {
            title: next_argument(arguments, argument)?,
            rules: parse_folder_rules(argument, next_argument(arguments, argument)?)?,
        }),
        "--folder-rename" => Ok(FolderMaintenance::Rename {
            folder: parse_folder_id(argument, next_argument(arguments, argument)?)?,
            title: next_argument(arguments, argument)?,
        }),
        "--folder-reorder" => Ok(FolderMaintenance::Reorder {
            folder: parse_folder_id(argument, next_argument(arguments, argument)?)?,
            position: parse_position(argument, next_argument(arguments, argument)?)?,
        }),
        "--folder-share" => Ok(FolderMaintenance::Share {
            folder: parse_folder_id(argument, next_argument(arguments, argument)?)?,
        }),
        "--folder-delete" => Ok(FolderMaintenance::Delete {
            folder: parse_folder_id(argument, next_argument(arguments, argument)?)?,
        }),
        "--folder-rules" => Ok(FolderMaintenance::Rules {
            folder: parse_folder_id(argument, next_argument(arguments, argument)?)?,
            rules: parse_folder_rules(argument, next_argument(arguments, argument)?)?,
        }),
        _ => UnknownArgumentSnafu {
            argument: argument.to_owned(),
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
