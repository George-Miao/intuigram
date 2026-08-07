//! Layered configuration for Intuigram.

mod credentials;

use std::path::PathBuf;
use std::{fmt, ops};

pub use credentials::{Error as CredentialError, save_application_credentials};
use figment::Figment;
use figment::providers::{Env, Format, Json, Serialized, Toml, Yaml};
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

const DEFAULT_MEDIA_CACHE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Failure produced while resolving configuration.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// A configured source could not be merged or deserialized.
    #[snafu(display("failed to resolve Intuigram configuration"))]
    Resolve {
        /// Underlying Figment failure.
        #[snafu(source(from(figment::Error, Box::new)))]
        source: Box<figment::Error>,
    },
}

/// Result returned by configuration operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Fully resolved Intuigram configuration.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Config {
    /// Filesystem locations used by Intuigram.
    pub paths: Paths,

    /// Media cache policy.
    pub media: Media,

    /// Telegram application and login settings.
    pub telegram: Telegram,

    /// Terminal presentation settings.
    pub view: View,
}

/// Terminal presentation settings.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct View {
    /// Density used by Chat, Message, and Folder presentation.
    pub mode: ViewMode,
}

/// Configurable terminal presentation density.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    /// Readable spacing with separation between list items.
    #[default]
    Default,

    /// Original dense presentation.
    Compact,
}

/// Telegram settings supplied by the user rather than embedded in Intuigram.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Telegram {
    /// Telegram application ID from my.telegram.org.
    pub api_id: Option<i32>,
    /// Telegram application hash from my.telegram.org.
    pub api_hash: Option<ApiHash>,
    /// Phone number used when a new authorization is required.
    pub phone_number: Option<String>,
}

/// Secret Telegram application hash whose diagnostics are always redacted.
#[derive(Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ApiHash(String);

impl ApiHash {
    /// Borrows the hash for authentication without allocating another copy.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiHash([REDACTED])")
    }
}

impl ops::Deref for ApiHash {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.expose()
    }
}

/// Filesystem locations used by Intuigram.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Paths {
    /// Directory containing durable databases.
    pub data: PathBuf,
    /// Directory containing redownloadable cache data.
    pub cache: PathBuf,
    /// Default destination for downloads.
    pub downloads: PathBuf,
}

/// Media cache policy.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Media {
    /// Maximum number of cached media bytes.
    pub cache_bytes: u64,
}

/// Platform-derived defaults supplied by the executable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformDefaults {
    /// Directory containing configuration files.
    pub config: PathBuf,
    /// Directory containing durable databases.
    pub data: PathBuf,
    /// Directory containing redownloadable cache data.
    pub cache: PathBuf,
    /// Default destination for downloads.
    pub downloads: PathBuf,
}

/// Optional values supplied by command-line arguments.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Overrides {
    /// Override for the durable data directory.
    pub data: Option<PathBuf>,
    /// Override for the redownloadable cache directory.
    pub cache: Option<PathBuf>,
    /// Override for the default download directory.
    pub downloads: Option<PathBuf>,
    /// Override for the maximum media cache size.
    pub media_cache_bytes: Option<u64>,
}

#[derive(Serialize)]
struct OverrideSource {
    paths: PathOverrides,
    media: MediaOverrides,
}

#[derive(Serialize)]
struct PathOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    cache: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    downloads: Option<PathBuf>,
}

#[derive(Serialize)]
struct MediaOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_bytes: Option<u64>,
}

/// Loads layered Intuigram configuration.
pub struct ConfigLoader {
    defaults: PlatformDefaults,
    environment: bool,
    overrides: Overrides,
}

impl ConfigLoader {
    /// Creates a loader using platform-derived filesystem defaults.
    #[must_use]
    pub const fn new(defaults: PlatformDefaults) -> Self {
        Self {
            defaults,
            environment: true,
            overrides: Overrides {
                data: None,
                cache: None,
                downloads: None,
                media_cache_bytes: None,
            },
        }
    }

    /// Enables or disables environment configuration.
    #[must_use]
    pub const fn read_environment(mut self, read: bool) -> Self {
        self.environment = read;
        self
    }

    /// Adds command-line values as the highest-priority source.
    #[must_use]
    pub fn with_overrides(mut self, overrides: Overrides) -> Self {
        self.overrides = overrides;
        self
    }

    /// Resolves the configured sources.
    pub fn load(self) -> Result<Config> {
        let defaults = Config {
            paths: Paths {
                data: self.defaults.data.clone(),
                cache: self.defaults.cache.clone(),
                downloads: self.defaults.downloads.clone(),
            },
            media: Media {
                cache_bytes: DEFAULT_MEDIA_CACHE_BYTES,
            },
            telegram: Telegram::default(),
            view: View {
                mode: ViewMode::Default,
            },
        };
        let mut figment = Figment::from(Serialized::defaults(defaults))
            .merge(Toml::file(self.defaults.config.join("config.toml")))
            .merge(Yaml::file(self.defaults.config.join("config.yaml")))
            .merge(Yaml::file(self.defaults.config.join("config.yml")))
            .merge(Json::file(self.defaults.config.join("config.json")))
            .merge(Toml::file(self.defaults.config.join("credentials.toml")));
        if self.environment {
            figment = figment.merge(Env::prefixed("INTUIGRAM_").split("__"));
        }
        let overrides = OverrideSource {
            paths: PathOverrides {
                data: self.overrides.data,
                cache: self.overrides.cache,
                downloads: self.overrides.downloads,
            },
            media: MediaOverrides {
                cache_bytes: self.overrides.media_cache_bytes,
            },
        };
        figment
            .merge(Serialized::defaults(overrides))
            .extract()
            .context(ResolveSnafu)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;

    use super::{ConfigLoader, Overrides, PlatformDefaults, ViewMode};

    fn defaults(root: &Path) -> PlatformDefaults {
        PlatformDefaults {
            config: root.join("config"),
            data: root.join("default-data"),
            cache: root.join("default-cache"),
            downloads: root.join("default-downloads"),
        }
    }

    #[test]
    fn json_overrides_yaml_and_toml_from_the_default_config_directory() {
        let temporary = tempdir().expect("temporary directory should be created");
        let platform = defaults(temporary.path());
        fs::create_dir_all(&platform.config).expect("config directory should be created");
        fs::write(
            platform.config.join("config.toml"),
            "[media]\ncache_bytes = 1024\n",
        )
        .expect("TOML config should be written");
        fs::write(
            platform.config.join("config.yaml"),
            "media:\n  cache_bytes: 2048\n",
        )
        .expect("YAML config should be written");
        fs::write(
            platform.config.join("config.json"),
            "{\"media\": {\"cache_bytes\": 4096}}",
        )
        .expect("JSON config should be written");

        let config = ConfigLoader::new(platform)
            .read_environment(false)
            .load()
            .expect("layered configuration should load");

        assert_eq!(config.media.cache_bytes, 4096);
    }

    #[test]
    fn command_line_values_override_configuration_files() {
        let temporary = tempdir().expect("temporary directory should be created");
        let platform = defaults(temporary.path());
        fs::create_dir_all(&platform.config).expect("config directory should be created");
        fs::write(
            platform.config.join("config.toml"),
            "[paths]\ndata = 'from-file'\n[media]\ncache_bytes = 'invalid lower source'\n",
        )
        .expect("TOML config should be written");
        let command_line_data = temporary.path().join("from-command-line");

        let config = ConfigLoader::new(platform)
            .read_environment(false)
            .with_overrides(Overrides {
                data: Some(command_line_data.clone()),
                media_cache_bytes: Some(8192),
                ..Overrides::default()
            })
            .load()
            .expect("layered configuration should load");

        assert_eq!(config.paths.data, command_line_data);
        assert_eq!(config.media.cache_bytes, 8192);
    }

    #[test]
    fn telegram_api_hash_is_loaded_but_redacted_from_diagnostics() {
        let temporary = tempdir().expect("temporary directory should be created");
        let platform = defaults(temporary.path());
        fs::create_dir_all(&platform.config).expect("config directory should be created");
        fs::write(
            platform.config.join("config.toml"),
            "[telegram]\napi_id = 42\napi_hash = 'super-secret-hash'\n",
        )
        .expect("TOML config should be written");

        let config = ConfigLoader::new(platform)
            .read_environment(false)
            .load()
            .expect("Telegram configuration should load");

        let hash = config
            .telegram
            .api_hash
            .expect("Telegram API hash should exist");
        assert_eq!(hash.expose(), "super-secret-hash");
        assert_eq!(format!("{hash:?}"), "ApiHash([REDACTED])");
    }

    #[test]
    fn spacious_view_is_default_and_compact_view_can_be_configured() {
        let temporary = tempdir().expect("temporary directory should be created");
        let platform = defaults(temporary.path());
        fs::create_dir_all(&platform.config).expect("config directory should be created");

        let default = ConfigLoader::new(platform.clone())
            .read_environment(false)
            .load()
            .expect("default configuration should load");
        assert_eq!(default.view.mode, ViewMode::Default);

        fs::write(
            platform.config.join("config.toml"),
            "[view]\nmode = 'compact'\n",
        )
        .expect("TOML config should be written");
        let compact = ConfigLoader::new(platform)
            .read_environment(false)
            .load()
            .expect("compact view configuration should load");
        assert_eq!(compact.view.mode, ViewMode::Compact);
    }
}
