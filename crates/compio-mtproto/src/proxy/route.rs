use std::io;
use std::net::{IpAddr, SocketAddr};

use compio::net::{TcpStream, ToSocketAddrsAsync};
use compio::time::timeout;

use super::types::{
    DnsStrategy, Error, Proxy, ProxyEndpoint, Route, RouteFailure, RoutesUnavailableSnafu,
    TargetAddress, endpoint, label,
};
use super::{handshake, mtproxy, padded};
use crate::{AbridgedConnection, BoxedTransport};

/// Opens the first available configured route to one of the Telegram
/// endpoints, then returns the transport and selected endpoint.
///
/// The function tries endpoints in their supplied order. The slice must
/// contain at least one endpoint.
pub async fn connect_route(
    telegram: &[SocketAddr],
    dc_id: i32,
    route: &Route,
) -> Result<(BoxedTransport, SocketAddr), Error> {
    let targets = telegram
        .iter()
        .copied()
        .map(TargetAddress::Address)
        .collect::<Vec<_>>();
    let (transport, target) = connect_targets(&targets, dc_id, route).await?;
    let TargetAddress::Address(endpoint) = target else {
        unreachable!("socket endpoint routes retain a socket endpoint")
    };
    Ok((transport, endpoint))
}

/// Opens a route to an IP or domain Telegram endpoint. SOCKS5 routes honor
/// their explicit local or remote DNS setting for domain endpoints.
pub async fn connect_route_target(
    telegram: TargetAddress,
    dc_id: i32,
    route: &Route,
) -> Result<BoxedTransport, Error> {
    connect_targets(std::slice::from_ref(&telegram), dc_id, route)
        .await
        .map(|(transport, _)| transport)
}

async fn connect_targets(
    telegram: &[TargetAddress],
    dc_id: i32,
    route: &Route,
) -> Result<(BoxedTransport, TargetAddress), Error> {
    let mut failures = Vec::new();
    if telegram.is_empty() {
        failures.push(RouteFailure {
            route: "Telegram endpoints".to_owned(),
            source: invalid("Telegram endpoint list is empty"),
        });
    }
    for proxy in &route.proxies {
        let proxy_addresses = match resolve_proxy(proxy, route, &mut failures).await {
            Some(addresses) => addresses,
            None => continue,
        };
        for target in telegram {
            let targets = match resolve_proxy_targets(target, proxy, route, &mut failures).await {
                Some(targets) => targets,
                None => continue,
            };
            for target in targets {
                for proxy_address in &proxy_addresses {
                    let description = format!(
                        "{} {proxy_address} to {}",
                        label(proxy),
                        target_label(&target)
                    );
                    match timeout(
                        route.timeout,
                        connect_proxy(*proxy_address, &target, dc_id, proxy),
                    )
                    .await
                    {
                        Ok(Ok(transport)) => return Ok((transport, target)),
                        Ok(Err(source)) => failures.push(RouteFailure {
                            route: description,
                            source,
                        }),
                        Err(_) => failures.push(RouteFailure {
                            route: description,
                            source: timed_out("proxy connection attempt timed out"),
                        }),
                    }
                }
            }
        }
    }
    if route.direct_fallback {
        for target in telegram {
            let addresses = match resolve_direct_target(target, route, &mut failures).await {
                Some(addresses) => addresses,
                None => continue,
            };
            for address in addresses {
                let description = format!("direct {address}");
                match timeout(route.timeout, AbridgedConnection::connect(address)).await {
                    Ok(Ok(connection)) => {
                        return Ok((
                            BoxedTransport::new(connection),
                            TargetAddress::Address(address),
                        ));
                    }
                    Ok(Err(source)) => failures.push(RouteFailure {
                        route: description,
                        source: io::Error::other(source),
                    }),
                    Err(_) => failures.push(RouteFailure {
                        route: description,
                        source: timed_out("direct connection attempt timed out"),
                    }),
                }
            }
        }
    }
    RoutesUnavailableSnafu { failures }.fail()
}

async fn resolve_proxy(
    proxy: &Proxy,
    route: &Route,
    failures: &mut Vec<RouteFailure>,
) -> Option<Vec<SocketAddr>> {
    let proxy = endpoint(proxy);
    match timeout(route.timeout, resolve_endpoint(proxy)).await {
        Ok(Ok(addresses)) => Some(addresses),
        Ok(Err(source)) => {
            failures.push(RouteFailure {
                route: format!("proxy DNS {}:{}", proxy.host, proxy.port),
                source,
            });
            None
        }
        Err(_) => {
            failures.push(RouteFailure {
                route: format!("proxy DNS {}:{}", proxy.host, proxy.port),
                source: timed_out("proxy DNS resolution timed out"),
            });
            None
        }
    }
}

async fn resolve_proxy_targets(
    target: &TargetAddress,
    proxy: &Proxy,
    route: &Route,
    failures: &mut Vec<RouteFailure>,
) -> Option<Vec<TargetAddress>> {
    let local_dns = matches!(
        proxy,
        Proxy::Socks5 {
            dns: DnsStrategy::Local,
            ..
        }
    );
    if !local_dns || matches!(target, TargetAddress::Address(_)) {
        return Some(vec![target.clone()]);
    }
    let TargetAddress::Domain(host, port) = target else {
        unreachable!("an unresolved target is a domain")
    };
    match timeout(route.timeout, resolve_host(host, *port)).await {
        Ok(Ok(addresses)) => Some(addresses.into_iter().map(TargetAddress::Address).collect()),
        Ok(Err(source)) => {
            failures.push(RouteFailure {
                route: format!("SOCKS5 target DNS {host}:{port}"),
                source,
            });
            None
        }
        Err(_) => {
            failures.push(RouteFailure {
                route: format!("SOCKS5 target DNS {host}:{port}"),
                source: timed_out("SOCKS5 target DNS resolution timed out"),
            });
            None
        }
    }
}

async fn resolve_direct_target(
    target: &TargetAddress,
    route: &Route,
    failures: &mut Vec<RouteFailure>,
) -> Option<Vec<SocketAddr>> {
    match target {
        TargetAddress::Address(address) => Some(vec![*address]),
        TargetAddress::Domain(host, port) => {
            match timeout(route.timeout, resolve_host(host, *port)).await {
                Ok(Ok(addresses)) => Some(addresses),
                Ok(Err(source)) => {
                    failures.push(RouteFailure {
                        route: format!("direct DNS {host}:{port}"),
                        source,
                    });
                    None
                }
                Err(_) => {
                    failures.push(RouteFailure {
                        route: format!("direct DNS {host}:{port}"),
                        source: timed_out("direct DNS resolution timed out"),
                    });
                    None
                }
            }
        }
    }
}

async fn resolve_endpoint(endpoint: &ProxyEndpoint) -> io::Result<Vec<SocketAddr>> {
    match endpoint.host.parse::<IpAddr>() {
        Ok(ip) => Ok(vec![SocketAddr::new(ip, endpoint.port)]),
        Err(_) => resolve_host(&endpoint.host, endpoint.port).await,
    }
}

async fn resolve_host(host: &str, port: u16) -> io::Result<Vec<SocketAddr>> {
    let addresses = ToSocketAddrsAsync::to_socket_addrs_async(&(host, port)).await?;
    unique_addresses(addresses)
}

pub(super) fn unique_addresses(
    addresses: impl IntoIterator<Item = SocketAddr>,
) -> io::Result<Vec<SocketAddr>> {
    let mut unique = Vec::new();
    for address in addresses {
        if !unique.contains(&address) {
            unique.push(address);
        }
    }
    if unique.is_empty() {
        Err(invalid("DNS resolution returned no addresses"))
    } else {
        Ok(unique)
    }
}

async fn connect_proxy(
    proxy_address: SocketAddr,
    telegram: &TargetAddress,
    dc_id: i32,
    proxy: &Proxy,
) -> io::Result<BoxedTransport> {
    let mut stream = TcpStream::connect(proxy_address).await?;
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

fn target_label(target: &TargetAddress) -> String {
    match target {
        TargetAddress::Address(address) => address.to_string(),
        TargetAddress::Domain(host, port) => format!("{host}:{port}"),
    }
}

fn timed_out(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, message)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
