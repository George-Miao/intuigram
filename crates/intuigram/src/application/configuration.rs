pub(super) fn telegram_credentials(config: &Config) -> Result<ApplicationCredentials> {
    let api_id = config
        .telegram
        .api_id
        .context(MissingTelegramSettingSnafu {
            setting: "telegram.api_id",
        })?;
    let api_hash = config
        .telegram
        .api_hash
        .as_ref()
        .context(MissingTelegramSettingSnafu {
            setting: "telegram.api_hash",
        })?;
    Ok(ApplicationCredentials::new(api_id, api_hash.expose()))
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
        let destination = match argument.as_str() {
            "-h" | "--help" => {
                parsed.help = true;
                continue;
            }
            "--config-dir" => &mut parsed.config,
            "--data-dir" => &mut parsed.data,
            "--cache-dir" => &mut parsed.cache,
            "--downloads-dir" => &mut parsed.downloads,
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
           -h, --help              Print this help\n\n\
         Configure telegram.api_id and telegram.api_hash in config.toml, YAML, JSON, or the\n\
         INTUIGRAM_TELEGRAM__API_ID and INTUIGRAM_TELEGRAM__API_HASH environment variables."
    );
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
use super::*;
