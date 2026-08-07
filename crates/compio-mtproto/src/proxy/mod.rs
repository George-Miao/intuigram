//! Proxy negotiation and MTProxy obfuscation for MTProto transports.

mod handshake;
mod mtproxy;
mod padded;
mod route;
mod types;

pub use route::{connect_route, connect_route_target};
pub use types::{
    DnsStrategy, Error, MtProxySecret, Proxy, ProxyCredentials, ProxyEndpoint, Route, TargetAddress,
};
