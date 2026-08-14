//! Layered configuration for Intuigram.

mod credentials;
mod proxy;
#[cfg(test)]
mod tests;

use std::num::NonZeroU16;
use std::path::PathBuf;
use std::{env, fmt, ops};

pub use credentials::{Error as CredentialError, save_application_credentials};
use figment::Figment;
use figment::providers::{Env, Format, Json, Serialized, Toml, Yaml};
pub use proxy::{
    Connection, DnsStrategy, Proxy, ProxyAuthentication, ProxyEnvironmentError, ProxySecret,
};
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

const DEFAULT_MEDIA_CACHE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const DEFAULT_MESSAGE_MAX_WIDTH: u16 = 96;

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

    /// A generic proxy environment variable contained an unusable URL.
    #[snafu(display("{variable} contains an invalid proxy URL"))]
    ProxyEnvironment {
        /// Environment variable that supplied the URL.
        variable: &'static str,

        /// Proxy URL validation failure.
        source: ProxyEnvironmentError,
    },

    /// A generic proxy environment variable was not Unicode.
    #[snafu(display("{variable} is not valid Unicode"))]
    ProxyEnvironmentEncoding {
        /// Environment variable containing the invalid value.
        variable: &'static str,
    },
}

/// Result returned by configuration operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Fully resolved Intuigram configuration.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Config {
    /// Telegram transport routing and fallback policy.
    pub connection: Connection,

    /// Optional encryption and unlock policy for Account-local data.
    pub local_lock: LocalLock,

    /// Diagnostic file logging.
    pub logging: Logging,

    /// Filesystem locations used by Intuigram.
    pub paths: Paths,

    /// Optional external programs used for platform workflows.
    pub external: External,

    /// Media cache policy.
    pub media: Media,

    /// Telegram application and login settings.
    pub telegram: Telegram,

    /// Terminal presentation settings.
    pub view: View,
}

/// Optional external program integrations.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct External {
    /// Program that prints one selected local path to standard output.
    pub path_picker: Option<ExternalCommand>,
}

/// One directly executed external program without shell interpretation.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ExternalCommand {
    /// Executable name or exact path.
    pub program: String,

    /// Literal arguments passed to the executable.
    #[serde(default)]
    pub args: Vec<String>,
}

/// Optional encryption of Account records and authorization material.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct LocalLock {
    /// Whether Account databases are encrypted with SQLCipher.
    pub enabled: bool,

    /// Where Intuigram obtains the unlock secret.
    pub unlock: UnlockMethod,
}

/// Source used to unlock encrypted Account databases.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlockMethod {
    /// Ask for a hidden passphrase on every launch.
    #[default]
    Passphrase,

    /// Store a random database key in the operating-system credential vault.
    Keyring,
}

/// File logging configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Logging {
    /// Explicit log path, or `None` to write `intuigram.log` in the data
    /// directory.
    pub path: Option<PathBuf>,
}

/// Terminal presentation settings.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct View {
    /// Density used by Chat, Message, and Folder presentation.
    pub mode: ViewMode,

    /// Maximum terminal-cell width used by one Message body and its metadata.
    pub message_max_width: NonZeroU16,
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

impl Config {
    /// Returns the configured log path or its data-directory default.
    #[must_use]
    pub fn log_path(&self) -> PathBuf {
        self.logging
            .path
            .clone()
            .unwrap_or_else(|| self.paths.data.join("intuigram.log"))
    }
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

#[derive(Serialize)]
struct GenericProxySource {
    connection: GenericProxyConnection,
}

#[derive(Serialize)]
struct GenericProxyConnection {
    proxies: [Proxy; 1],
    direct_fallback: bool,
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
            connection: Connection::default(),

            local_lock: LocalLock::default(),

            logging: Logging::default(),

            paths: Paths {
                data: self.defaults.data.clone(),
                cache: self.defaults.cache.clone(),
                downloads: self.defaults.downloads.clone(),
            },

            external: External::default(),
            media: Media {
                cache_bytes: DEFAULT_MEDIA_CACHE_BYTES,
            },
            telegram: Telegram::default(),
            view: View {
                mode: ViewMode::Default,
                message_max_width: NonZeroU16::new(DEFAULT_MESSAGE_MAX_WIDTH)
                    .expect("the default Message width is nonzero"),
            },
        };
        let mut figment = Figment::from(Serialized::defaults(defaults))
            .merge(Toml::file(self.defaults.config.join("config.toml")))
            .merge(Yaml::file(self.defaults.config.join("config.yaml")))
            .merge(Yaml::file(self.defaults.config.join("config.yml")))
            .merge(Json::file(self.defaults.config.join("config.json")))
            .merge(Toml::file(self.defaults.config.join("credentials.toml")));
        if self.environment {
            if let Some(source) = generic_proxy_source()? {
                figment = figment.merge(Serialized::defaults(source));
            }
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

fn generic_proxy_source() -> Result<Option<GenericProxySource>> {
    const VARIABLES: [&str; 6] = [
        "all_proxy",
        "ALL_PROXY",
        "https_proxy",
        "HTTPS_PROXY",
        "http_proxy",
        "HTTP_PROXY",
    ];

    for variable in VARIABLES {
        match env::var(variable) {
            Ok(value) if value.trim().is_empty() => {}
            Ok(value) => {
                let proxy = proxy::environment_proxy(value.trim())
                    .context(ProxyEnvironmentSnafu { variable })?;
                return Ok(Some(GenericProxySource {
                    connection: GenericProxyConnection {
                        proxies: [proxy],
                        direct_fallback: false,
                    },
                }));
            }
            Err(env::VarError::NotPresent) => {}
            Err(env::VarError::NotUnicode(_)) => {
                return ProxyEnvironmentEncodingSnafu { variable }.fail();
            }
        }
    }
    Ok(None)
}
