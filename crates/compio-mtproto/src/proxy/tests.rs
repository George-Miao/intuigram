use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
use std::thread::JoinHandle;
use std::time::Duration;

use super::route::unique_addresses;
use super::*;

#[test]
fn dns_results_keep_resolver_order() {
    let ipv6 = SocketAddr::from((Ipv6Addr::LOCALHOST, 443));
    let ipv4 = SocketAddr::from((Ipv4Addr::LOCALHOST, 443));

    let addresses = unique_addresses([ipv6, ipv4, ipv6]).expect("DNS fixture should resolve");

    assert_eq!(addresses, [ipv6, ipv4]);
}

#[test]
fn direct_ipv6_endpoint_connects() {
    let listener =
        TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).expect("IPv6 loopback listener should bind");
    let endpoint = listener
        .local_addr()
        .expect("listener address should exist");

    let (_, selected) = runtime()
        .block_on(connect_route(&[endpoint], 2, &Route::default()))
        .expect("IPv6 direct route should connect");

    assert_eq!(selected, endpoint);
}

#[test]
fn direct_fallback_reaches_ipv4_endpoint() {
    let listener =
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("IPv4 loopback listener should bind");
    let ipv4 = listener
        .local_addr()
        .expect("listener address should exist");
    let unavailable_ipv6 = SocketAddr::from((Ipv6Addr::LOCALHOST, ipv4.port()));

    let (_, selected) = runtime()
        .block_on(connect_route(
            &[unavailable_ipv6, ipv4],
            2,
            &Route::default(),
        ))
        .expect("direct route should fall back to IPv4");

    assert_eq!(selected, ipv4);
}

#[test]
fn http_proxy_accepts_ipv6_destination() {
    let (proxy, request) = http_proxy();
    let telegram = "[2001:67c:4e8:f002::a]:443"
        .parse()
        .expect("Telegram IPv6 fixture should parse");
    let route = proxy_route(Proxy::HttpConnect {
        endpoint: proxy_endpoint(proxy),
        credentials: None,
    });

    let (_, selected) = runtime()
        .block_on(connect_route(&[telegram], 2, &route))
        .expect("HTTP CONNECT route should connect");
    let request = String::from_utf8(request.join().expect("proxy should finish"))
        .expect("HTTP request should be text");

    assert_eq!(selected, telegram);
    assert!(request.starts_with("CONNECT [2001:67c:4e8:f002::a]:443 HTTP/1.1\r\n"));
}

#[test]
fn socks_proxy_accepts_ipv6_destination() {
    let (proxy, request) = socks_proxy();
    let telegram_ip = "2001:67c:4e8:f002::a"
        .parse::<Ipv6Addr>()
        .expect("Telegram IPv6 fixture should parse");
    let telegram = SocketAddr::from((telegram_ip, 443));
    let route = proxy_route(Proxy::Socks5 {
        endpoint: proxy_endpoint(proxy),
        credentials: None,
        dns: DnsStrategy::Remote,
    });

    let (_, selected) = runtime()
        .block_on(connect_route(&[telegram], 2, &route))
        .expect("SOCKS5 route should connect");
    let request = request.join().expect("proxy should finish");

    assert_eq!(selected, telegram);
    assert_eq!(&request[..4], &[5, 1, 0, 4]);
    assert_eq!(&request[4..20], &telegram_ip.octets());
    assert_eq!(&request[20..], &443_u16.to_be_bytes());
}

#[test]
fn mtproxy_accepts_ipv6_server_endpoint() {
    let listener =
        TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).expect("IPv6 loopback listener should bind");
    let proxy = listener
        .local_addr()
        .expect("listener address should exist");
    let header = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("MTProxy should connect");
        let mut header = [0_u8; 64];
        stream
            .read_exact(&mut header)
            .expect("MTProxy should send an obfuscated header");
        header
    });
    let telegram = SocketAddr::from((Ipv4Addr::new(149, 154, 167, 41), 443));
    let route = proxy_route(Proxy::MtProxy {
        endpoint: proxy_endpoint(proxy),
        secret: MtProxySecret::parse("00112233445566778899aabbccddeeff")
            .expect("MTProxy secret should parse"),
    });

    let (_, selected) = runtime()
        .block_on(connect_route(&[telegram], 2, &route))
        .expect("MTProxy route should connect through IPv6");

    assert_eq!(selected, telegram);
    assert_ne!(header.join().expect("proxy should finish"), [0_u8; 64]);
}

#[test]
fn route_failures_keep_endpoint_order() {
    let first = "[2001:db8::1]:443"
        .parse()
        .expect("IPv6 fixture should parse");
    let second = SocketAddr::from((Ipv4Addr::new(192, 0, 2, 1), 443));
    let (proxy, server) = rejecting_http_proxy(2);
    let route = proxy_route(Proxy::HttpConnect {
        endpoint: proxy_endpoint(proxy),
        credentials: None,
    });

    let result = runtime().block_on(connect_route(&[first, second], 2, &route));
    server.join().expect("proxy should finish");
    let Err(error) = result else {
        panic!("rejected endpoints should fail");
    };
    let message = error.to_string();

    let first_position = message
        .find(&first.to_string())
        .expect("IPv6 failure should be reported");
    let second_position = message
        .find(&second.to_string())
        .expect("IPv4 failure should be reported");
    assert!(first_position < second_position);
}

fn proxy_route(proxy: Proxy) -> Route {
    Route {
        proxies: vec![proxy],
        direct_fallback: false,
        timeout: Duration::from_secs(1),
    }
}

fn proxy_endpoint(address: SocketAddr) -> ProxyEndpoint {
    ProxyEndpoint {
        host: address.ip().to_string(),
        port: address.port(),
    }
}

fn http_proxy() -> (SocketAddr, JoinHandle<Vec<u8>>) {
    let listener =
        TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).expect("IPv6 HTTP proxy should bind");
    let endpoint = listener
        .local_addr()
        .expect("listener address should exist");
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("HTTP client should connect");
        let request = read_http_request(&mut stream);
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .expect("HTTP response should write");
        request
    });
    (endpoint, thread)
}

fn rejecting_http_proxy(attempts: usize) -> (SocketAddr, JoinHandle<()>) {
    let listener =
        TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).expect("IPv6 HTTP proxy should bind");
    let endpoint = listener
        .local_addr()
        .expect("listener address should exist");
    let thread = std::thread::spawn(move || {
        for _ in 0..attempts {
            let (mut stream, _) = listener.accept().expect("HTTP client should connect");
            let _ = read_http_request(&mut stream);
            stream
                .write_all(b"HTTP/1.1 403 Forbidden\r\n\r\n")
                .expect("HTTP rejection should write");
        }
    });
    (endpoint, thread)
}

fn read_http_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    while !request.ends_with(b"\r\n\r\n") {
        let mut byte = [0_u8; 1];
        stream
            .read_exact(&mut byte)
            .expect("HTTP request should read");
        request.push(byte[0]);
    }
    request
}

fn socks_proxy() -> (SocketAddr, JoinHandle<Vec<u8>>) {
    let listener =
        TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).expect("IPv6 SOCKS proxy should bind");
    let endpoint = listener
        .local_addr()
        .expect("listener address should exist");
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("SOCKS client should connect");
        let mut greeting = [0_u8; 3];
        stream
            .read_exact(&mut greeting)
            .expect("SOCKS greeting should read");
        assert_eq!(greeting, [5, 1, 0]);
        stream
            .write_all(&[5, 0])
            .expect("SOCKS greeting should write");
        let mut request = vec![0_u8; 22];
        stream
            .read_exact(&mut request)
            .expect("SOCKS request should read");
        let mut response = vec![5, 0, 0, 4];
        response.extend([0_u8; 16]);
        response.extend(0_u16.to_be_bytes());
        stream
            .write_all(&response)
            .expect("SOCKS response should write");
        request
    });
    (endpoint, thread)
}

fn runtime() -> compio::runtime::Runtime {
    compio::runtime::Runtime::new().expect("test runtime should initialize")
}
