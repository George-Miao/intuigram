use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

use snafu::Snafu;

/// Whether a domain target is resolved by Intuigram or by a capable proxy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DnsStrategy {
    /// Send domain targets to the proxy without local resolution.
    #[default]
    Remote,

    /// Resolve domain targets locally before proxy negotiation.
    Local,
}

/// A proxy server address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyEndpoint {
    /// DNS name or literal IP of the proxy server.
    pub host: String,

    /// TCP port of the proxy server.
    pub port: u16,
}

/// Optional username/password proxy authentication.
#[derive(Clone, Eq, PartialEq)]
pub struct ProxyCredentials {
    /// Authentication username.
    pub username: String,

    /// Authentication password.
    pub password: String,
}

impl fmt::Debug for ProxyCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProxyCredentials")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

/// Parsed 16-byte MTProxy secret.
#[derive(Clone, Eq, PartialEq)]
pub struct MtProxySecret {
    pub(crate) bytes: [u8; 16],
    pub(crate) transport: MtProxyTransport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MtProxyTransport {
    Abridged,
    PaddedIntermediate,
}

impl MtProxySecret {
    /// Parses a bare 32-hex secret or the random-padding `dd` form.
    pub fn parse(value: &str) -> Result<Self> {
        let (transport, hex) = value
            .strip_prefix("dd")
            .map_or((MtProxyTransport::Abridged, value), |hex| {
                (MtProxyTransport::PaddedIntermediate, hex)
            });
        if hex.len() != 32 || value.starts_with("ee") {
            return InvalidSecretSnafu.fail();
        }
        let mut bytes = [0_u8; 16];
        for (index, byte) in bytes.iter_mut().enumerate() {
            let offset = index * 2;
            *byte = u8::from_str_radix(&hex[offset..offset + 2], 16)
                .map_err(|_| Error::InvalidSecret)?;
        }
        Ok(Self { bytes, transport })
    }
}

impl fmt::Debug for MtProxySecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MtProxySecret([REDACTED])")
    }
}

/// One configured proxy transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Proxy {
    /// SOCKS5 tunnel, optionally using RFC 1929 authentication.
    Socks5 {
        /// Proxy server.
        endpoint: ProxyEndpoint,

        /// Optional username/password authentication.
        credentials: Option<ProxyCredentials>,

        /// Domain-target resolution policy.
        dns: DnsStrategy,
    },

    /// HTTP/1.1 CONNECT tunnel, optionally using Basic authentication.
    HttpConnect {
        /// Proxy server.
        endpoint: ProxyEndpoint,

        /// Optional username/password authentication.
        credentials: Option<ProxyCredentials>,
    },

    /// Telegram MTProxy with obfuscated abridged or padded-intermediate
    /// transport.
    MtProxy {
        /// Proxy server.
        endpoint: ProxyEndpoint,

        /// Shared MTProxy secret.
        secret: MtProxySecret,
    },
}

/// Ordered connection policy with optional direct fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Route {
    /// Proxy attempts in priority order.
    pub proxies: Vec<Proxy>,

    /// Try a direct connection after all proxy attempts fail.
    pub direct_fallback: bool,

    /// Maximum duration of each connect and negotiation attempt.
    pub timeout: Duration,
}

impl Default for Route {
    fn default() -> Self {
        Self {
            proxies: Vec::new(),
            direct_fallback: true,
            timeout: Duration::from_secs(10),
        }
    }
}

/// One failed connection route.
#[derive(Debug)]
pub struct RouteFailure {
    /// Safe route description containing no credentials or secrets.
    pub route: String,

    /// Concrete transport or negotiation failure.
    pub source: std::io::Error,
}

/// Proxy connection failure.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// An MTProxy secret was malformed or used an unsupported TLS-emulation
    /// form.
    #[snafu(display("MTProxy secret must be 32 hexadecimal digits, optionally prefixed by dd"))]
    InvalidSecret,

    /// Every configured transport route failed.
    #[snafu(display(
        "all {} Telegram transport routes failed: {}",
        failures.len(),
        summarize(failures)
    ))]
    RoutesUnavailable {
        /// Failures in attempted order.
        failures: Vec<RouteFailure>,
    },
}

/// Result returned by proxy operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

fn summarize(failures: &[RouteFailure]) -> String {
    failures
        .iter()
        .map(|failure| format!("{} ({})", failure.route, failure.source))
        .collect::<Vec<_>>()
        .join("; ")
}

pub(crate) fn endpoint(proxy: &Proxy) -> &ProxyEndpoint {
    match proxy {
        Proxy::Socks5 { endpoint, .. }
        | Proxy::HttpConnect { endpoint, .. }
        | Proxy::MtProxy { endpoint, .. } => endpoint,
    }
}

pub(crate) fn label(proxy: &Proxy) -> &'static str {
    match proxy {
        Proxy::Socks5 { .. } => "SOCKS5",
        Proxy::HttpConnect { .. } => "HTTP CONNECT",
        Proxy::MtProxy { .. } => "MTProxy",
    }
}

/// Telegram endpoint supplied to a direct or proxy route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TargetAddress {
    /// Already-resolved IP endpoint.
    Address(SocketAddr),

    /// Domain endpoint whose resolution follows the route policy.
    Domain(String, u16),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mtproxy_secret_accepts_bare_and_random_padding_forms() {
        let bare = MtProxySecret::parse("00112233445566778899aabbccddeeff")
            .expect("bare secret should parse");
        let padded = MtProxySecret::parse("dd00112233445566778899aabbccddeeff")
            .expect("dd secret should parse");
        assert_eq!(bare.bytes, padded.bytes);
        assert_eq!(bare.transport, MtProxyTransport::Abridged);
        assert_eq!(padded.transport, MtProxyTransport::PaddedIntermediate);
        assert!(MtProxySecret::parse("ee00112233445566778899aabbccddeeff").is_err());
        assert!(!format!("{bare:?}").contains("001122"));
    }

    #[test]
    fn direct_transport_is_the_safe_default_route() {
        let route = Route::default();
        assert!(route.proxies.is_empty());
        assert!(route.direct_fallback);
    }
}
