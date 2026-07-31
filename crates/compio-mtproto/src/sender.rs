use grammers_crypto::DequeBuffer;
use grammers_mtproto::MsgId;
use grammers_mtproto::mtp::{Deserialization, Encrypted, Mtp};
use grammers_tl_types::{Deserializable, RemoteCall, Serializable};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::{AbridgedConnection, AuthKeyMaterial};

const MAXIMUM_ENVELOPE_BYTES: usize = (1024 * 1024) + (8 * 1024);
const LEADING_SPACE: usize = grammers_mtproto::mtp::ENCRYPTED_PACKET_HEADER_LEN
    + grammers_mtproto::mtp::MESSAGE_CONTAINER_HEADER_LEN;
const MAX_BAD_MESSAGE_RETRIES: usize = 3;

/// Failure while invoking an encrypted Telegram RPC.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// The direct `MTProto` transport failed.
    #[snafu(display("MTProto transport failed during encrypted invocation"))]
    Transport {
        /// Underlying transport failure.
        source: crate::TransportError,
    },

    /// The encrypted response envelope was invalid.
    #[snafu(display("invalid encrypted MTProto response"))]
    DeserializeEnvelope {
        /// Underlying envelope failure.
        source: grammers_mtproto::mtp::DeserializeError,
    },

    /// The response body had the wrong TL constructor or was truncated.
    #[snafu(display("invalid TL response body"))]
    DeserializeResponse {
        /// Underlying TL failure.
        source: grammers_tl_types::deserialize::Error,
    },

    /// Telegram returned an RPC-level error.
    #[snafu(display("Telegram RPC error {code}: {message}"))]
    Rpc {
        /// Telegram RPC error code.
        code: i32,
        /// Telegram RPC error message.
        message: String,
    },

    /// Telegram rejected the outgoing envelope repeatedly.
    #[snafu(display("Telegram repeatedly rejected an outgoing MTProto message: {description}"))]
    BadMessage {
        /// Human-readable `MTProto` rejection reason.
        description: &'static str,
    },

    /// Telegram returned an unrecoverable deserialization failure.
    #[snafu(display("Telegram response could not be matched to the request"))]
    ResponseFailure,

    /// The request was too large for a bounded `MTProto` envelope.
    #[snafu(display("Telegram request does not fit the bounded MTProto envelope"))]
    RequestTooLarge,
}

/// Result returned by encrypted invocations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Returns whether this is a Telegram RPC error with the given message.
    #[must_use]
    pub fn is_rpc(&self, expected: &str) -> bool {
        matches!(self, Self::Rpc { message, .. } if message == expected)
    }
}

enum RequestOutcome {
    Body(Vec<u8>),
    Retry(&'static str),
}

/// Sequential encrypted RPC connection with owned `MTProto` session state.
pub struct EncryptedConnection {
    transport: AbridgedConnection,
    mtp: Encrypted,
    pending_updates: Vec<Vec<u8>>,
}

impl EncryptedConnection {
    /// Starts encrypted messaging with freshly generated authorization
    /// material.
    #[must_use]
    pub fn new(transport: AbridgedConnection, material: &AuthKeyMaterial) -> Self {
        let mtp = Encrypted::build()
            .time_offset(material.time_offset)
            .first_salt(material.first_salt)
            .finish(material.auth_key);
        Self {
            transport,
            mtp,
            pending_updates: Vec::new(),
        }
    }

    /// Invokes one Telegram method while continuing to acknowledge and retain
    /// updates.
    pub async fn invoke<R>(&mut self, request: &R) -> Result<R::Return>
    where
        R: RemoteCall + Serializable,
        R::Return: Deserializable,
    {
        let body = request.to_bytes();
        let mut last_bad_message = "unknown bad-message response";
        for _ in 0..MAX_BAD_MESSAGE_RETRIES {
            let request_id = self.send_request(&body).await?;
            match self.receive_until_result(request_id).await? {
                RequestOutcome::Body(body) => {
                    return R::Return::from_bytes(&body).context(DeserializeResponseSnafu);
                }
                RequestOutcome::Retry(description) => last_bad_message = description,
            }
        }
        BadMessageSnafu {
            description: last_bad_message,
        }
        .fail()
    }

    /// Takes raw update constructors received while waiting for RPC results.
    pub fn take_updates(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.pending_updates)
    }

    /// Returns the current authorization key for durable session persistence.
    #[must_use]
    pub fn auth_key(&self) -> [u8; 256] {
        self.mtp.auth_key()
    }

    fn push_envelope(&mut self, body: &[u8]) -> Result<(MsgId, Vec<u8>)> {
        let mut envelope = DequeBuffer::with_capacity(MAXIMUM_ENVELOPE_BYTES, LEADING_SPACE);
        let request_id = self
            .mtp
            .push(&mut envelope, body)
            .context(RequestTooLargeSnafu)?;
        self.mtp
            .finalize(&mut envelope)
            .expect("an accepted request produces a non-empty encrypted envelope");
        Ok((request_id, envelope[..].to_vec()))
    }

    async fn send_request(&mut self, body: &[u8]) -> Result<MsgId> {
        let (request_id, envelope) = self.push_envelope(body)?;
        self.transport
            .send(envelope)
            .await
            .context(TransportSnafu)?;
        Ok(request_id)
    }

    async fn receive_until_result(&mut self, request_id: MsgId) -> Result<RequestOutcome> {
        loop {
            let mut envelope = self.transport.receive().await.context(TransportSnafu)?;
            let results = self
                .mtp
                .deserialize(&mut envelope)
                .context(DeserializeEnvelopeSnafu)?;
            self.send_service_envelope().await?;

            for result in results {
                match result {
                    Deserialization::RpcResult(result) if result.msg_id == request_id => {
                        return Ok(RequestOutcome::Body(result.body));
                    }
                    Deserialization::RpcError(result) if result.msg_id == request_id => {
                        return RpcSnafu {
                            code: result.error.error_code,
                            message: result.error.error_message,
                        }
                        .fail();
                    }
                    Deserialization::BadMessage(result) if result.msg_id == request_id => {
                        if result.retryable() {
                            return Ok(RequestOutcome::Retry(result.description()));
                        }
                        return BadMessageSnafu {
                            description: result.description(),
                        }
                        .fail();
                    }
                    Deserialization::Failure(result) if result.msg_id == request_id => {
                        return ResponseFailureSnafu.fail();
                    }
                    Deserialization::OwnUpdate { update, .. } | Deserialization::Update(update) => {
                        self.pending_updates.push(update);
                    }
                    Deserialization::RpcResult(_)
                    | Deserialization::RpcError(_)
                    | Deserialization::BadMessage(_)
                    | Deserialization::Failure(_) => {}
                }
            }
        }
    }

    async fn send_service_envelope(&mut self) -> Result<()> {
        let mut service = DequeBuffer::with_capacity(MAXIMUM_ENVELOPE_BYTES, LEADING_SPACE);
        if self.mtp.finalize(&mut service).is_some() {
            let service = service[..].to_vec();
            self.transport.send(service).await.context(TransportSnafu)?;
        }
        Ok(())
    }
}
