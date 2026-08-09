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
