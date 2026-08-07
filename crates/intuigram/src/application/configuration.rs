use super::*;

pub(super) fn resolve_telegram_credentials(
    config: &Config,
    config_directory: &std::path::Path,
) -> Result<ApplicationCredentials> {
    if let (Some(api_id), Some(api_hash)) =
        (config.telegram.api_id, config.telegram.api_hash.as_ref())
    {
        return Ok(ApplicationCredentials::new(api_id, api_hash.expose()));
    }
    println!(
        "First-run setup\n\nIntuigram public builds do not bundle shared Telegram application credentials.\nCreate your own application at https://my.telegram.org/apps, then enter its values below.\nThe API hash is hidden and both values will be saved to an owner-protected credentials.toml."
    );
    let api_id = match config.telegram.api_id {
        Some(api_id) => api_id,
        None => prompt("Application ID", "Telegram application ID")?
            .parse::<i32>()
            .ok()
            .filter(|value| *value > 0)
            .context(InvalidApplicationIdSnafu)?,
    };
    let api_hash = match config.telegram.api_hash.as_ref() {
        Some(api_hash) => api_hash.expose().to_owned(),
        None => rpassword::prompt_password("Application hash (hidden): ")
            .context(PromptApplicationHashSnafu)?,
    };
    let path = intuigram_config::save_application_credentials(config_directory, api_id, &api_hash)
        .context(SaveApplicationCredentialsSnafu)?;
    println!("Saved credentials to {}.", path.display());
    Ok(ApplicationCredentials::new(api_id, api_hash))
}

pub(super) fn store_session(session: &Session) -> SessionMaterial {
    SessionMaterial::new(
        session.dc_id,
        session.endpoint.to_string(),
        session.auth_key(),
        session.time_offset,
        session.first_salt,
    )
}

pub(super) fn telegram_session(session: &SessionMaterial) -> Result<Session> {
    let endpoint = session.endpoint.parse().context(InvalidEndpointSnafu {
        endpoint: session.endpoint.clone(),
    })?;
    Ok(Session::new(
        session.dc_id,
        endpoint,
        session.auth_key(),
        session.time_offset,
        session.first_salt,
    ))
}

pub(super) fn prompt(label: &str, field: &'static str) -> Result<String> {
    print!("{label}: ");
    io::stdout().flush().context(PromptSnafu { field })?;
    let mut value = String::new();
    io::stdin()
        .read_line(&mut value)
        .context(PromptSnafu { field })?;
    let value = value.trim().to_owned();
    if value.is_empty() {
        return EmptyPromptSnafu { field }.fail();
    }
    Ok(value)
}

pub(super) fn parse_arguments(arguments: impl IntoIterator<Item = String>) -> Result<Arguments> {
    let mut parsed = Arguments::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if matches!(
            argument.as_str(),
            "--media-cache-usage"
                | "--clear-media-cache"
                | "--clear-account-data"
                | "--remove-account"
                | "--logout"
        ) {
            if parsed.maintenance.is_some() {
                return ConflictingMaintenanceSnafu.fail();
            }
            let value = arguments
                .next()
                .ok_or_else(|| Error::MissingArgumentValue {
                    argument: argument.clone(),
                })?;
            let account = value
                .parse::<i64>()
                .ok()
                .and_then(AccountId::new)
                .ok_or_else(|| Error::InvalidArgumentValue {
                    argument: argument.clone(),
                    value,
                })?;
            parsed.maintenance = Some(match argument.as_str() {
                "--media-cache-usage" => Maintenance::MediaUsage(account),
                "--clear-media-cache" => Maintenance::ClearMedia(account),
                "--clear-account-data" => Maintenance::ClearAccount(account),
                "--remove-account" => Maintenance::ClearAccount(account),
                "--logout" => Maintenance::Logout(account),
                _ => unreachable!("maintenance arguments were matched above"),
            });
            continue;
        }
        if matches!(
            argument.as_str(),
            "--schedule-message"
                | "--scheduled-list"
                | "--scheduled-edit"
                | "--scheduled-reschedule"
                | "--scheduled-delete"
                | "--scheduled-send-now"
        ) {
            if parsed.maintenance.is_some() {
                return ConflictingMaintenanceSnafu.fail();
            }
            let account =
                parse_account_argument(&argument, next_argument(&mut arguments, &argument)?)?;
            let command = parse_scheduled_maintenance(&mut arguments, &argument)?;
            parsed.maintenance = Some(Maintenance::Scheduled(account, command));
            continue;
        }
        if argument.starts_with("--folder-") {
            if parsed.maintenance.is_some() {
                return ConflictingMaintenanceSnafu.fail();
            }
            let account =
                parse_account_argument(&argument, next_argument(&mut arguments, &argument)?)?;
            let command = parse_folder_maintenance(&mut arguments, &argument)?;
            parsed.maintenance = Some(Maintenance::Folder(account, command));
            continue;
        }
        if matches!(
            argument.as_str(),
            "--media-browse"
                | "--media-send"
                | "--media-file"
                | "--record-media"
                | "--send-contact"
        ) {
            if parsed.maintenance.is_some() {
                return ConflictingMaintenanceSnafu.fail();
            }
            let account =
                parse_account_argument(&argument, next_argument(&mut arguments, &argument)?)?;
            let command = parse_media_maintenance(&mut arguments, &argument)?;
            parsed.maintenance = Some(Maintenance::RichMedia(account, command));
            continue;
        }
        let destination = match argument.as_str() {
            "-h" | "--help" => {
                parsed.help = true;
                continue;
            }
            "--config-dir" => &mut parsed.config,
            "--data-dir" => &mut parsed.data,
            "--cache-dir" => &mut parsed.cache,
            "--downloads-dir" => &mut parsed.downloads,
            "--account" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| Error::MissingArgumentValue {
                        argument: argument.clone(),
                    })?;
                parsed.account = Some(parse_account_argument(&argument, value)?);
                continue;
            }
            "--add-account" => {
                parsed.add_account = true;
                continue;
            }
            "--list-accounts" => {
                parsed.list_accounts = true;
                continue;
            }
            _ => return UnknownArgumentSnafu { argument }.fail(),
        };
        *destination = Some(
            arguments
                .next()
                .ok_or_else(|| Error::MissingArgumentValue {
                    argument: argument.clone(),
                })?
                .into(),
        );
    }
    if parsed.account.is_some() && parsed.add_account {
        return ConflictingAccountSelectionSnafu.fail();
    }
    Ok(parsed)
}

pub(super) fn platform_defaults(config_override: Option<PathBuf>) -> Result<PlatformDefaults> {
    let config = match config_override {
        Some(path) => path,
        None => dirs::config_dir()
            .context(MissingPlatformDirectorySnafu {
                kind: "configuration",
            })?
            .join("intuigram"),
    };
    let data = dirs::data_dir()
        .context(MissingPlatformDirectorySnafu { kind: "data" })?
        .join("intuigram");
    let cache = dirs::cache_dir()
        .context(MissingPlatformDirectorySnafu { kind: "cache" })?
        .join("intuigram");
    let downloads =
        dirs::download_dir().context(MissingPlatformDirectorySnafu { kind: "downloads" })?;
    Ok(PlatformDefaults {
        config,
        data,
        cache,
        downloads,
    })
}

pub(super) fn print_help() {
    println!(
        "Intuigram terminal client\n\n\
         Usage: intuigram [OPTIONS]\n\n\
         Options:\n\
           --config-dir PATH       Override the platform config directory\n\
           --data-dir PATH         Override the platform data directory\n\
           --cache-dir PATH        Override the platform cache directory\n\
           --downloads-dir PATH    Override the platform Downloads directory\n\
           --account ID            Switch to a registered Telegram Account\n\
           --add-account           Authorize and add another Telegram Account\n\
           --list-accounts         List registered Accounts without opening the TUI\n\
           --media-cache-usage ID  Show one Account's cache usage and configured limit\n\
           --clear-media-cache ID  Clear only redownloadable media for one Account\n\
           --clear-account-data ID Clear local records, authorization, and media after confirmation\n\
           --remove-account ID     Remove local data; server authorization may remain active\n\
           --logout ID             Revoke Telegram authorization, then remove local Account data\n\
           --folder-create ID TITLE RULES\n\
                                   Create a Folder; RULES is a comma-separated rule list\n\
           --folder-rename ID FOLDER TITLE\n\
                                   Rename a custom Folder\n\
           --folder-reorder ID FOLDER POSITION\n\
                                   Move a custom Folder to a zero-based position\n\
           --folder-share ID FOLDER\n\
                                   Export a share link for explicitly included Chats\n\
           --folder-delete ID FOLDER\n\
                                   Delete a Folder without deleting its Chats\n\
           --folder-rules ID FOLDER RULES\n\
                                   Replace inclusion/exclusion category rules\n\
           --media-browse ID KIND QUERY\n\
                                   Browse stickers, gifs, or custom-emoji; use - for no query\n\
           --media-send ID CHAT KIND INDEX QUERY\n\
                                   Send an item from the same media-library query\n\
           --media-file ID CHAT KIND PATH\n\
                                   Send voice, video-note, sticker, gif, or custom-emoji media\n\
           --record-media ID CHAT KIND SECONDS DEVICE\n\
                                   Record voice or video-note with ffmpeg, then send it\n\
           --send-contact ID CHAT PHONE FIRST LAST\n\
                                   Share a Telegram contact card\n\
           --schedule-message ID CHAT RFC3339 TEXT\n\
                                   Schedule text at a time carrying an explicit UTC offset\n\
           --scheduled-list ID CHAT\n\
                                   List Telegram-owned Scheduled Messages for a Chat\n\
           --scheduled-edit ID CHAT MESSAGE TEXT\n\
                                   Replace a Scheduled Message's text\n\
           --scheduled-reschedule ID CHAT MESSAGE RFC3339\n\
                                   Change its delivery time using an explicit UTC offset\n\
           --scheduled-delete ID CHAT MESSAGE\n\
                                   Delete a Scheduled Message after confirmation\n\
           --scheduled-send-now ID CHAT MESSAGE\n\
                                   Ask Telegram to send a Scheduled Message immediately\n\
           -h, --help              Print this help\n\n\
         Configure telegram.api_id and telegram.api_hash in config.toml, YAML, JSON, or the\n\
         INTUIGRAM_TELEGRAM__API_ID and INTUIGRAM_TELEGRAM__API_HASH environment variables."
    );
}

fn parse_account_argument(argument: &str, value: String) -> Result<AccountId> {
    value
        .parse::<i64>()
        .ok()
        .and_then(AccountId::new)
        .ok_or_else(|| Error::InvalidArgumentValue {
            argument: argument.to_owned(),
            value,
        })
}

pub(super) fn mime_type_for_path(path: &std::path::Path) -> String {
    match path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("gif") => "image/gif",
        Some("jpeg" | "jpg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("mp4") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("pdf") => "application/pdf",
        Some("txt" | "md") => "text/plain",
        _ => "application/octet-stream",
    }
    .to_owned()
}

pub(super) fn derived_random_id(base: i64, index: usize, domain: u64) -> i64 {
    let index = u64::try_from(index).unwrap_or(u64::MAX);
    let mut value = (base as u64) ^ domain ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    (value ^ (value >> 31)) as i64
}
