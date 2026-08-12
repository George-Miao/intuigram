use super::*;

#[cfg(test)]
mod tests;

pub(super) fn resolve_telegram_credentials(
    config: &Config,
    config_directory: &std::path::Path,
) -> Result<ApplicationCredentials> {
    if let (Some(api_id), Some(api_hash)) =
        (config.telegram.api_id, config.telegram.api_hash.as_ref())
    {
        return Ok(ApplicationCredentials::new(api_id, api_hash.expose()));
    }
    let mut ui = LoginUi::enter().context(TerminalSnafu)?;
    let mut api_id = config.telegram.api_id;
    let mut api_id_text = api_id.map_or_else(String::new, |value| value.to_string());
    let mut api_hash = config
        .telegram
        .api_hash
        .as_ref()
        .map_or_else(String::new, |value| value.expose().to_owned());
    let mut field = if api_id.is_some() {
        LoginField::ApplicationHash
    } else {
        LoginField::ApplicationId
    };
    let mut error = None;
    loop {
        let (label, value, secret, can_go_back) = match field {
            LoginField::ApplicationId => ("Application ID", &api_id_text, false, false),
            LoginField::ApplicationHash => (
                "Application hash",
                &api_hash,
                true,
                config.telegram.api_id.is_none(),
            ),
            _ => unreachable!("credential setup has exactly two fields"),
        };
        let input = ui
            .read(
                LoginPrompt {
                    field,
                    label,
                    description: "Create an application at https://my.telegram.org/apps. Values \
                                  are saved with owner-only permissions.",
                    error: error.as_deref(),
                    secret,
                    can_go_back,
                },
                value,
            )
            .context(TerminalSnafu)?;
        error = None;
        match (field, input) {
            (_, LoginInput::Cancel) => return LoginCancelledSnafu.fail(),
            (LoginField::ApplicationHash, LoginInput::Back) => {
                api_hash.clear();
                field = LoginField::ApplicationId;
            }
            (LoginField::ApplicationId, LoginInput::Submit(value)) => {
                api_id_text = value;
                api_id = api_id_text
                    .trim()
                    .parse::<i32>()
                    .ok()
                    .filter(|value| *value > 0);
                if api_id.is_some() {
                    field = LoginField::ApplicationHash;
                } else {
                    error = Some("Application ID must be a positive decimal integer".to_owned());
                }
            }
            (LoginField::ApplicationHash, LoginInput::Submit(value)) => {
                api_hash = value;
                if api_hash.is_empty() {
                    error = Some("Application hash must not be empty".to_owned());
                } else {
                    break;
                }
            }
            (_, LoginInput::Back) => {}
            _ => unreachable!("credential form only visits application fields"),
        }
    }
    drop(ui);
    let api_id = api_id.context(InvalidApplicationIdSnafu)?;
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
