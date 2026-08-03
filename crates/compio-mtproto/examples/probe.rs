use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use compio_mtproto::{AbridgedConnection, BoxedTransport, generate_auth_key};
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
enum ProbeError {
    #[snafu(display("direct Telegram transport failed"))]
    Connect {
        source: compio_mtproto::TransportError,
    },

    #[snafu(display("Telegram authorization-key exchange failed"))]
    KeyExchange {
        source: compio_mtproto::KeyExchangeError,
    },
}

fn main() {
    let endpoint = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(149, 154, 167, 50)), 443);
    let runtime = match compio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("failed to start Compio runtime: {error}");
            std::process::exit(1);
        }
    };
    let result = runtime.block_on(async {
        let connection = AbridgedConnection::connect(endpoint)
            .await
            .context(ConnectSnafu)?;
        let mut connection = BoxedTransport::new(connection);
        generate_auth_key(&mut connection)
            .await
            .context(KeyExchangeSnafu)
    });
    match result {
        Ok(material) => println!(
            "MTProto authorization-key exchange succeeded (time offset {}, salt received)",
            material.time_offset
        ),
        Err(error) => {
            eprintln!("MTProto probe failed: {error}");
            std::process::exit(1);
        }
    }
}
