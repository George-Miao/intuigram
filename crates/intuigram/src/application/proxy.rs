use std::time::Duration;

use compio_mtproto::{DnsStrategy, MtProxySecret, Proxy, ProxyCredentials, ProxyEndpoint, Route};
use intuigram_config::{DnsStrategy as ConfigDns, Proxy as ConfigProxy};
use snafu::ResultExt;

use super::{Config, ProxyConfigurationSnafu, Result};

pub(super) fn telegram_route(config: &Config) -> Result<Route> {
    let proxies = config
        .connection
        .proxies
        .iter()
        .map(proxy)
        .collect::<Result<Vec<_>>>()?;
    Ok(Route {
        proxies,
        direct_fallback: config.connection.direct_fallback,
        timeout: Duration::from_secs(config.connection.timeout_seconds.max(1)),
    })
}

fn proxy(config: &ConfigProxy) -> Result<Proxy> {
    Ok(match config {
        ConfigProxy::Socks5 {
            host,
            port,
            auth,
            dns,
        } => Proxy::Socks5 {
            endpoint: endpoint(host, *port),
            credentials: auth.as_ref().map(credentials),
            dns: match dns {
                ConfigDns::Remote => DnsStrategy::Remote,
                ConfigDns::Local => DnsStrategy::Local,
            },
        },
        ConfigProxy::HttpConnect { host, port, auth } => Proxy::HttpConnect {
            endpoint: endpoint(host, *port),
            credentials: auth.as_ref().map(credentials),
        },
        ConfigProxy::MtProxy { host, port, secret } => Proxy::MtProxy {
            endpoint: endpoint(host, *port),
            secret: MtProxySecret::parse(secret.expose()).context(ProxyConfigurationSnafu)?,
        },
    })
}

fn endpoint(host: &str, port: u16) -> ProxyEndpoint {
    ProxyEndpoint {
        host: host.to_owned(),
        port,
    }
}

fn credentials(auth: &intuigram_config::ProxyAuthentication) -> ProxyCredentials {
    ProxyCredentials {
        username: auth.username.clone(),
        password: auth.password.expose().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_mtproxy_secret_is_rejected_before_network_work() {
        assert!(MtProxySecret::parse("not-a-secret").is_err());
    }
}
