use grammers_crypto::DequeBuffer;
use grammers_mtproto::authentication;
use grammers_mtproto::mtp::{Deserialization, Mtp, Plain};
use grammers_tl_types::{Cursor, Deserializable, RemoteCall, Serializable};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::AbridgedConnection;

/// Fresh `MTProto` authorization material produced by the DH exchange.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthKeyMaterial {
    /// Secret 2048-bit authorization key. Never log this value.
    pub auth_key: [u8; 256],
    /// Initial difference between local and Telegram server time.
    pub time_offset: i32,
    /// Initial server salt for encrypted envelopes.
    pub first_salt: i64,
}

/// Failure while generating a fresh `MTProto` authorization key.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// The direct transport failed during key exchange.
    #[snafu(display("MTProto transport failed during authorization-key exchange"))]
    Transport {
        /// Underlying transport failure.
        source: crate::TransportError,
    },

    /// The audited Diffie-Hellman state machine rejected Telegram's response.
    #[snafu(display("MTProto authorization-key exchange rejected a response"))]
    Authentication {
        /// Underlying authentication state-machine failure.
        source: authentication::Error,
    },

    /// A plaintext response envelope was invalid.
    #[snafu(display("invalid plaintext MTProto response during key exchange"))]
    PlainEnvelope {
        /// Underlying `MTProto` envelope failure.
        source: grammers_mtproto::mtp::DeserializeError,
    },

    /// Telegram returned no matching plaintext RPC result.
    #[snafu(display("Telegram returned no plaintext RPC result during key exchange"))]
    MissingRpcResult,

    /// Telegram returned more than one plaintext result for one request.
    #[snafu(display("Telegram returned multiple plaintext RPC results during key exchange"))]
    MultipleRpcResults,

    /// A key-exchange response had the wrong TL constructor or was truncated.
    #[snafu(display("invalid TL response during authorization-key exchange"))]
    DeserializeResponse {
        /// Underlying TL failure.
        source: grammers_tl_types::deserialize::Error,
    },
}

/// Result returned by authorization-key generation.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Completes Telegram's four-request authorization-key handshake.
pub async fn generate_auth_key(connection: &mut AbridgedConnection) -> Result<AuthKeyMaterial> {
    let mut plain = Plain::new();

    let (request, step1) = authentication::step1().context(AuthenticationSnafu)?;
    let response = invoke_plain(connection, &mut plain, &request).await?;

    let (request, step2) = authentication::step2(step1, response).context(AuthenticationSnafu)?;
    let response = invoke_plain(connection, &mut plain, &request).await?;

    let (request, step3) = authentication::step3(step2, response).context(AuthenticationSnafu)?;
    let response = invoke_plain(connection, &mut plain, &request).await?;

    let finished = authentication::create_key(step3, response).context(AuthenticationSnafu)?;
    Ok(AuthKeyMaterial {
        auth_key: finished.auth_key,
        time_offset: finished.time_offset,
        first_salt: finished.first_salt,
    })
}

async fn invoke_plain<R>(
    connection: &mut AbridgedConnection,
    plain: &mut Plain,
    request: &R,
) -> Result<R::Return>
where
    R: RemoteCall + Serializable,
    R::Return: Deserializable,
{
    let request = request.to_bytes();
    let mut envelope = DequeBuffer::with_capacity(0, 32 + request.len());
    plain
        .push(&mut envelope, &request)
        .expect("fresh plaintext envelope accepts exactly one request");
    plain
        .finalize(&mut envelope)
        .expect("plaintext request envelope is non-empty");
    connection
        .send(envelope[..].to_vec())
        .await
        .context(TransportSnafu)?;

    let mut response = connection.receive().await.context(TransportSnafu)?;
    let results = plain
        .deserialize(&mut response)
        .context(PlainEnvelopeSnafu)?;
    let mut bodies = results.into_iter().filter_map(|result| match result {
        Deserialization::RpcResult(result) => Some(result.body),
        _ => None,
    });
    let body = bodies.next().context(MissingRpcResultSnafu)?;
    if bodies.next().is_some() {
        return MultipleRpcResultsSnafu.fail();
    }
    R::Return::deserialize(&mut Cursor::from_slice(&body)).context(DeserializeResponseSnafu)
}
