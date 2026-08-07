use std::fmt;

use serde::{Deserialize, Serialize};

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
