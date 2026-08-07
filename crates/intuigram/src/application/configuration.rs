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

#[cfg(test)]
mod argument_tests {
    use super::{Maintenance, parse_arguments};

    #[test]
    fn storage_maintenance_requires_one_positive_account_id() {
        let parsed = parse_arguments(["--media-cache-usage".to_owned(), "42".to_owned()])
            .expect("valid maintenance arguments should parse");
        assert!(matches!(parsed.maintenance, Some(Maintenance::MediaUsage(id)) if id.get() == 42));

        assert!(parse_arguments(["--clear-media-cache".to_owned(), "0".to_owned()]).is_err());
        assert!(
            parse_arguments([
                "--clear-media-cache".to_owned(),
                "1".to_owned(),
                "--clear-account-data".to_owned(),
                "1".to_owned(),
            ])
            .is_err()
        );
    }

    #[test]
    fn account_launcher_arguments_are_unambiguous() {
        let selected = parse_arguments(["--account".to_owned(), "42".to_owned()])
            .expect("Account selection should parse");
        assert_eq!(selected.account.map(|account| account.get()), Some(42));

        assert!(
            parse_arguments([
                "--account".to_owned(),
                "42".to_owned(),
                "--add-account".to_owned(),
            ])
            .is_err()
        );
    }
}
