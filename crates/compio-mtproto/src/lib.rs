//! Completion-based `MTProto` transport primitives.

mod abridged;
mod driver;
mod key_exchange;
mod proxy;
mod sender;
mod transport;

pub use abridged::{AbridgedCodec, AbridgedConnection, Error as TransportError};
pub use driver::{ConnectionDriver, Invocation, InvocationHandle, UpdateStream};
pub use key_exchange::{AuthKeyMaterial, Error as KeyExchangeError, generate_auth_key};
pub use proxy::{
    DnsStrategy, Error as ProxyError, MtProxySecret, Proxy, ProxyCredentials, ProxyEndpoint, Route,
    TargetAddress, connect_route, connect_route_target,
};
pub use sender::{EncryptedConnection, Error as InvocationError};
pub use transport::{BoxedTransport, Transport};
