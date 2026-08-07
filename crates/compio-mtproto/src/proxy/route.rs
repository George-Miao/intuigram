use std::net::SocketAddr;

use compio::net::TcpStream;
use compio::time::timeout;

use super::types::{
    Error, Proxy, Route, RouteFailure, RoutesUnavailableSnafu, TargetAddress, endpoint, label,
};
use super::{handshake, mtproxy, padded};
use crate::{AbridgedConnection, BoxedTransport};

/// Opens the first available configured proxy route, then optionally falls
/// back to direct TCP.
pub async fn connect_route(
    telegram: SocketAddr,
    dc_id: i32,
    route: &Route,
) -> Result<BoxedTransport, Error> {
    connect_route_target(TargetAddress::Address(telegram), dc_id, route).await
}

/// Opens a route to an IP or domain Telegram endpoint. SOCKS5 routes honor
/// their explicit local/remote DNS setting for domain endpoints.
pub async fn connect_route_target(
    telegram: TargetAddress,
    dc_id: i32,
    route: &Route,
) -> Result<BoxedTransport, Error> {
    let mut failures = Vec::new();
    for proxy in &route.proxies {
        let description = format!(
            "{} {}:{}",
            label(proxy),
            endpoint(proxy).host,
            endpoint(proxy).port
        );
        match timeout(route.timeout, connect_proxy(&telegram, dc_id, proxy)).await {
            Ok(Ok(transport)) => return Ok(transport),
            Ok(Err(source)) => failures.push(RouteFailure {
                route: description,
                source,
            }),
            Err(_) => failures.push(RouteFailure {
                route: description,
                source: std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "proxy connection attempt timed out",
                ),
            }),
        }
    }
    if route.direct_fallback {
        match timeout(route.timeout, connect_direct(&telegram)).await {
            Ok(Ok(connection)) => return Ok(BoxedTransport::new(connection)),
            Ok(Err(source)) => failures.push(RouteFailure {
                route: direct_label(&telegram),
                source: std::io::Error::other(source),
            }),
            Err(_) => failures.push(RouteFailure {
                route: direct_label(&telegram),
                source: std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "direct connection attempt timed out",
                ),
            }),
        }
    }
    RoutesUnavailableSnafu { failures }.fail()
}

async fn connect_proxy(
    telegram: &TargetAddress,
    dc_id: i32,
    proxy: &Proxy,
) -> std::io::Result<BoxedTransport> {
    let endpoint = endpoint(proxy);
    let mut stream = TcpStream::connect((endpoint.host.as_str(), endpoint.port)).await?;
    match proxy {
        Proxy::Socks5 {
            credentials, dns, ..
        } => {
            handshake::socks5(&mut stream, telegram, credentials.as_ref(), *dns).await?;
            Ok(BoxedTransport::new(AbridgedConnection::from_stream(stream)))
        }
        Proxy::HttpConnect { credentials, .. } => {
            handshake::http_connect(&mut stream, telegram, credentials.as_ref()).await?;
            Ok(BoxedTransport::new(AbridgedConnection::from_stream(stream)))
        }
        Proxy::MtProxy { secret, .. } => {
            let (reader, writer) = mtproxy::initialize(stream, dc_id, secret).await?;
            match secret.transport {
                super::types::MtProxyTransport::Abridged => Ok(BoxedTransport::new(
                    AbridgedConnection::from_halves(reader, writer, true),
                )),
                super::types::MtProxyTransport::PaddedIntermediate => {
                    Ok(BoxedTransport::new(padded::Connection::new(reader, writer)))
                }
            }
        }
    }
}

async fn connect_direct(target: &TargetAddress) -> crate::abridged::Result<AbridgedConnection> {
    match target {
        TargetAddress::Address(address) => AbridgedConnection::connect(*address).await,
        TargetAddress::Domain(host, port) => {
            let stream = TcpStream::connect((host.as_str(), *port))
                .await
                .map_err(crate::TransportError::from)?;
            Ok(AbridgedConnection::from_stream(stream))
        }
    }
}

fn direct_label(target: &TargetAddress) -> String {
    match target {
        TargetAddress::Address(address) => format!("direct {address}"),
        TargetAddress::Domain(host, port) => format!("direct {host}:{port}"),
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};
    use std::time::Duration;

    use super::*;

    #[test]
    fn secrets_and_passwords_are_redacted_from_route_diagnostics() {
        let proxy = Proxy::MtProxy {
            endpoint: super::super::types::ProxyEndpoint {
                host: "proxy.example".to_owned(),
                port: 443,
            },
            secret: super::super::types::MtProxySecret::parse("00112233445566778899aabbccddeeff")
                .expect("secret should parse"),
        };
        let debug = format!("{proxy:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("001122"));
    }

    #[test]
    fn direct_fallback_is_retained_after_ordered_proxy_attempts() {
        let route = Route {
            proxies: vec![Proxy::Socks5 {
                endpoint: super::super::types::ProxyEndpoint {
                    host: "proxy.example".to_owned(),
                    port: 1080,
                },
                credentials: None,
                dns: super::super::types::DnsStrategy::Remote,
            }],
            direct_fallback: true,
            timeout: Duration::from_millis(100),
        };
        let endpoint = TargetAddress::Address(SocketAddr::from((Ipv4Addr::LOCALHOST, 443)));
        let mut labels = route
            .proxies
            .iter()
            .map(|proxy| label(proxy).to_owned())
            .collect::<Vec<_>>();
        if route.direct_fallback {
            labels.push(direct_label(&endpoint));
        }
        assert_eq!(labels, ["SOCKS5", "direct 127.0.0.1:443"]);
    }
}
