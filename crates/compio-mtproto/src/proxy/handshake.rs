use std::io;

use base64::Engine;
use compio::buf::BufResult;
use compio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::types::{DnsStrategy, ProxyCredentials, TargetAddress};

pub(crate) async fn socks5<S>(
    stream: &mut S,
    target: &TargetAddress,
    credentials: Option<&ProxyCredentials>,
    dns: DnsStrategy,
) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite,
{
    let methods = if credentials.is_some() {
        vec![5, 2, 0, 2]
    } else {
        vec![5, 1, 0]
    };
    write_all(stream, methods).await?;
    let response = read_exact(stream, 2).await?;
    if response[0] != 5 || response[1] == 0xff {
        return Err(invalid("SOCKS5 proxy rejected authentication methods"));
    }
    if response[1] == 2 {
        authenticate_socks5(stream, credentials).await?;
    } else if response[1] != 0 {
        return Err(invalid(
            "SOCKS5 proxy selected an unsupported authentication method",
        ));
    }
    let request = socks_target(target, dns).await?;
    write_all(stream, request).await?;
    let header = read_exact(stream, 4).await?;
    if header[0] != 5 || header[1] != 0 {
        return Err(invalid("SOCKS5 proxy rejected the CONNECT request"));
    }
    let address_len = match header[3] {
        1 => 4,
        4 => 16,
        3 => usize::from(read_exact(stream, 1).await?[0]),
        _ => return Err(invalid("SOCKS5 proxy returned an invalid address type")),
    };
    let _ = read_exact(stream, address_len + 2).await?;
    Ok(())
}

async fn authenticate_socks5<S>(
    stream: &mut S,
    credentials: Option<&ProxyCredentials>,
) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite,
{
    let credentials = credentials.ok_or_else(|| invalid("SOCKS5 proxy requires credentials"))?;
    let username = credentials.username.as_bytes();
    let password = credentials.password.as_bytes();
    if username.len() > 255 || password.len() > 255 {
        return Err(invalid("SOCKS5 credentials exceed 255 bytes"));
    }
    let mut request = Vec::with_capacity(3 + username.len() + password.len());
    request.extend([1, username.len() as u8]);
    request.extend(username);
    request.push(password.len() as u8);
    request.extend(password);
    write_all(stream, request).await?;
    if read_exact(stream, 2).await? != [1, 0] {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "SOCKS5 username/password authentication failed",
        ));
    }
    Ok(())
}

async fn socks_target(target: &TargetAddress, dns: DnsStrategy) -> io::Result<Vec<u8>> {
    let mut request = vec![5, 1, 0];
    match target {
        TargetAddress::Address(address) => encode_address(&mut request, *address),
        TargetAddress::Domain(host, port) if dns == DnsStrategy::Remote => {
            if host.len() > 255 {
                return Err(invalid("SOCKS5 target hostname exceeds 255 bytes"));
            }
            request.extend([3, host.len() as u8]);
            request.extend(host.as_bytes());
            request.extend(port.to_be_bytes());
        }
        TargetAddress::Domain(host, port) => {
            let address =
                compio::net::ToSocketAddrsAsync::to_socket_addrs_async(&(host.as_str(), *port))
                    .await?
                    .next()
                    .ok_or_else(|| invalid("local DNS returned no SOCKS5 target address"))?;
            encode_address(&mut request, address);
        }
    }
    Ok(request)
}

fn encode_address(request: &mut Vec<u8>, address: std::net::SocketAddr) {
    match address.ip() {
        std::net::IpAddr::V4(ip) => {
            request.push(1);
            request.extend(ip.octets());
        }
        std::net::IpAddr::V6(ip) => {
            request.push(4);
            request.extend(ip.octets());
        }
    }
    request.extend(address.port().to_be_bytes());
}

pub(crate) async fn http_connect<S>(
    stream: &mut S,
    target: &TargetAddress,
    credentials: Option<&ProxyCredentials>,
) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite,
{
    let authority = match target {
        TargetAddress::Address(address) => address.to_string(),
        TargetAddress::Domain(host, port) => format!("{host}:{port}"),
    };
    let mut request = format!(
        "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\nProxy-Connection: Keep-Alive\r\n"
    );
    if let Some(credentials) = credentials {
        if credentials.username.contains(['\r', '\n'])
            || credentials.password.contains(['\r', '\n'])
        {
            return Err(invalid("HTTP proxy credentials contain a line break"));
        }
        let encoded = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", credentials.username, credentials.password));
        request.push_str(&format!("Proxy-Authorization: Basic {encoded}\r\n"));
    }
    request.push_str("\r\n");
    write_all(stream, request.into_bytes()).await?;
    let mut header = Vec::with_capacity(256);
    while !header.ends_with(b"\r\n\r\n") {
        if header.len() == 16 * 1024 {
            return Err(invalid("HTTP CONNECT response headers exceed 16 KiB"));
        }
        header.push(read_exact(stream, 1).await?[0]);
    }
    let first_line = header
        .split(|byte| *byte == b'\n')
        .next()
        .and_then(|line| std::str::from_utf8(line).ok())
        .ok_or_else(|| invalid("HTTP CONNECT response is not valid text"))?;
    let status = first_line
        .split_ascii_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| invalid("HTTP CONNECT response has no status code"))?;
    if !(200..300).contains(&status) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("HTTP CONNECT proxy returned status {status}"),
        ));
    }
    Ok(())
}

async fn read_exact<S: AsyncRead>(stream: &mut S, length: usize) -> io::Result<Vec<u8>> {
    let BufResult(result, buffer) = stream.read_exact(vec![0; length]).await;
    result.map(|()| buffer)
}

async fn write_all<S: AsyncWrite>(stream: &mut S, buffer: Vec<u8>) -> io::Result<()> {
    let BufResult(result, _) = stream.write_all(buffer).await;
    result
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};

    use super::*;

    #[test]
    fn socks5_negotiates_remote_dns_without_leaking_credentials() {
        let mut io = ScriptedIo::new(vec![5, 0, 5, 0, 0, 1, 127, 0, 0, 1, 0, 1]);
        runtime().block_on(async {
            socks5(
                &mut io,
                &TargetAddress::Domain("example.com".to_owned(), 443),
                None,
                DnsStrategy::Remote,
            )
            .await
            .expect("SOCKS negotiation");
        });
        assert_eq!(&io.outgoing[..3], &[5, 1, 0]);
        assert_eq!(&io.outgoing[3..8], &[5, 1, 0, 3, 11]);
        assert_eq!(&io.outgoing[8..19], b"example.com");
        assert_eq!(&io.outgoing[19..], &443_u16.to_be_bytes());
    }

    #[test]
    fn http_connect_sends_basic_auth_and_accepts_success() {
        let mut io = ScriptedIo::new(b"HTTP/1.1 200 Connection Established\r\n\r\n".to_vec());
        runtime().block_on(async {
            http_connect(
                &mut io,
                &TargetAddress::Address(SocketAddr::from((Ipv4Addr::new(149, 154, 167, 41), 443))),
                Some(&ProxyCredentials {
                    username: "user".to_owned(),
                    password: "pass".to_owned(),
                }),
            )
            .await
            .expect("HTTP CONNECT negotiation");
        });
        let request = String::from_utf8(io.outgoing).expect("ASCII HTTP request");
        assert!(request.starts_with("CONNECT 149.154.167.41:443 HTTP/1.1\r\n"));
        assert!(request.contains("Proxy-Authorization: Basic dXNlcjpwYXNz\r\n"));
    }

    struct ScriptedIo {
        incoming: &'static [u8],
        outgoing: Vec<u8>,
    }

    impl ScriptedIo {
        fn new(incoming: Vec<u8>) -> Self {
            Self {
                incoming: Box::leak(incoming.into_boxed_slice()),
                outgoing: Vec::new(),
            }
        }
    }

    impl AsyncRead for ScriptedIo {
        async fn read<B: compio::buf::IoBufMut>(&mut self, buffer: B) -> BufResult<usize, B> {
            self.incoming.read(buffer).await
        }
    }

    impl AsyncWrite for ScriptedIo {
        async fn write<B: compio::buf::IoBuf>(&mut self, buffer: B) -> BufResult<usize, B> {
            self.outgoing.write(buffer).await
        }

        async fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }

        async fn shutdown(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn runtime() -> compio::runtime::Runtime {
        compio::runtime::Runtime::new().expect("test runtime")
    }
}
