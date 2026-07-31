//! Completion-based `MTProto` transport primitives.

mod abridged;
mod key_exchange;
mod sender;

pub use abridged::{AbridgedCodec, AbridgedConnection, Error as TransportError};
pub use key_exchange::{AuthKeyMaterial, Error as KeyExchangeError, generate_auth_key};
pub use sender::{EncryptedConnection, Error as InvocationError};
