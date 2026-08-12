use std::fmt;

use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt, Snafu};
use url::Url;

/// Ordered proxy policy for Telegram connections.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Connection {
    /// Proxy routes tried in configuration order.
    pub proxies: Vec<Proxy>,

    /// Whether direct TCP is attempted after all proxies fail.
    pub direct_fallback: bool,

    /// Maximum seconds allowed for each route attempt.
    pub timeout_seconds: u64,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            proxies: Vec::new(),
            direct_fallback: true,
            timeout_seconds: 10,
        }
    }
}

/// Proxy transport configured by the user.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Proxy {
    /// SOCKS5 with explicit target DNS behavior.
    Socks5 {
        /// Proxy host or literal IP.
        host: String,

        /// Proxy TCP port.
        port: u16,

        /// Optional RFC 1929 credentials.
        auth: Option<ProxyAuthentication>,

        /// Where domain targets are resolved.
        #[serde(default)]
        dns: DnsStrategy,
    },

    /// HTTP/1.1 CONNECT tunnel.
    HttpConnect {
        /// Proxy host or literal IP.
        host: String,

        /// Proxy TCP port.
        port: u16,

        /// Optional Basic credentials.
        auth: Option<ProxyAuthentication>,
    },

    /// Telegram MTProxy using a hex-encoded shared secret.
    MtProxy {
        /// Proxy host or literal IP.
        host: String,

        /// Proxy TCP port.
        port: u16,

        /// Bare or `dd`-prefixed MTProxy secret.
        secret: ProxySecret,
    },
}

/// Username/password proxy authentication.
#[derive(Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct ProxyAuthentication {
    /// Authentication username.
    pub username: String,

    /// Authentication password.
    pub password: ProxySecret,
}

impl fmt::Debug for ProxyAuthentication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProxyAuthentication")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

/// Configuration secret whose diagnostics are always redacted.
#[derive(Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ProxySecret(String);

impl ProxySecret {
    /// Borrows the configured secret for the transport adapter.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ProxySecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProxySecret([REDACTED])")
    }
}

/// SOCKS5 target DNS policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DnsStrategy {
    /// Send domain targets to SOCKS5 for resolution.
    #[default]
    Remote,

    /// Resolve domain targets on the local machine.
    Local,
}

/// Failure while converting a conventional proxy URL into an Intuigram route.
#[derive(Debug, Snafu)]
pub enum ProxyEnvironmentError {
    /// The value was not a valid absolute URL.
    #[snafu(display("proxy URL is invalid"))]
    Parse {
        /// URL parser failure without the potentially secret input.
        source: url::ParseError,
    },

    /// The proxy transport is not supported.
    #[snafu(display("proxy URL uses unsupported scheme {scheme:?}"))]
    UnsupportedScheme {
        /// Unsupported URL scheme.
        scheme: String,
    },

    /// The URL did not identify a proxy host.
    #[snafu(display("proxy URL has no host"))]
    MissingHost,

    /// The URL included components with no proxy-route meaning.
    #[snafu(display("proxy URL path, query, and fragment must be empty"))]
    UnexpectedComponents,

    /// A percent-decoded username or password was not UTF-8.
    #[snafu(display("proxy URL credentials are not valid UTF-8"))]
    InvalidCredentialEncoding {
        /// UTF-8 validation failure without the potentially secret input.
        source: std::str::Utf8Error,
    },
}

pub(crate) fn environment_proxy(value: &str) -> Result<Proxy, ProxyEnvironmentError> {
    let url = Url::parse(value).context(ParseSnafu)?;
    if !matches!(url.path(), "" | "/") || url.query().is_some() || url.fragment().is_some() {
        return UnexpectedComponentsSnafu.fail();
    }
    let host = url.host_str().context(MissingHostSnafu)?.to_owned();
    let auth = authentication(&url)?;

    match url.scheme() {
        "socks5" => Ok(Proxy::Socks5 {
            host,
            port: url.port().unwrap_or(1080),
            auth,
            dns: DnsStrategy::Local,
        }),
        "socks5h" => Ok(Proxy::Socks5 {
            host,
            port: url.port().unwrap_or(1080),
            auth,
            dns: DnsStrategy::Remote,
        }),
        "http" => Ok(Proxy::HttpConnect {
            host,
            port: url.port().unwrap_or(80),
            auth,
        }),
        scheme => UnsupportedSchemeSnafu {
            scheme: scheme.to_owned(),
        }
        .fail(),
    }
}

fn authentication(url: &Url) -> Result<Option<ProxyAuthentication>, ProxyEnvironmentError> {
    if url.username().is_empty() && url.password().is_none() {
        return Ok(None);
    }
    Ok(Some(ProxyAuthentication {
        username: decode_credential(url.username())?,
        password: ProxySecret(decode_credential(url.password().unwrap_or_default())?),
    }))
}

fn decode_credential(value: &str) -> Result<String, ProxyEnvironmentError> {
    percent_decode_str(value)
        .decode_utf8()
        .context(InvalidCredentialEncodingSnafu)
        .map(|value| value.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socks5h_environment_url_uses_remote_dns() {
        let proxy = environment_proxy("socks5h://proxy.example").expect("proxy URL should parse");

        assert!(matches!(
            proxy,
            Proxy::Socks5 {
                host,
                port: 1080,
                auth: None,
                dns: DnsStrategy::Remote,
            } if host == "proxy.example"
        ));
    }

    #[test]
    fn encoded_environment_credentials_are_decoded() {
        let proxy = environment_proxy("http://user%20name:pass%2Fword@proxy.example:8080")
            .expect("proxy URL should parse");
        let Proxy::HttpConnect {
            auth: Some(auth), ..
        } = proxy
        else {
            panic!("HTTP proxy should retain credentials");
        };

        assert_eq!(auth.username, "user name");
        assert_eq!(auth.password.expose(), "pass/word");
        assert!(!format!("{auth:?}").contains("pass/word"));
    }

    #[test]
    fn unsupported_environment_scheme_is_rejected() {
        let error =
            environment_proxy("https://proxy.example").expect_err("TLS-to-proxy is not supported");

        assert!(error.to_string().contains("unsupported scheme \"https\""));
    }
}
