//! Telegram Abridged transport framing.

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use compio::buf::{IoBuf, IoBufMut, Slice};
use compio::io::framed::SymmetricFramed;
use compio::io::framed::codec::{Decoder, Encoder};
use compio::io::framed::frame::{Frame, Framer};
use compio::net::TcpStream;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::Transport;

const PREAMBLE: u8 = 0xef;
const LONG_HEADER: u8 = 0x7f;
const MAX_PAYLOAD_BYTES: usize = 0x00ff_ffff * 4;

/// Failure while framing or transporting abridged `MTProto` packets.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// The endpoint could not be reached.
    #[snafu(display("failed to connect MTProto transport to {endpoint}"))]
    Connect {
        /// Remote endpoint.
        endpoint: SocketAddr,

        /// Underlying network failure.
        source: io::Error,
    },

    /// An `MTProto` payload was not padded to a four-byte word.
    #[snafu(display("abridged payload length {length} is not divisible by four"))]
    UnalignedPayload {
        /// Invalid payload length.
        length: usize,
    },

    /// A received frame exceeded the protocol limit.
    #[snafu(display("abridged payload length {length} exceeds the protocol limit"))]
    PayloadTooLarge {
        /// Invalid payload length.
        length: usize,
    },

    /// Telegram returned a malformed abridged header.
    #[snafu(display("Telegram returned malformed abridged header byte {first:#04x}"))]
    MalformedHeader {
        /// Unexpected first header byte.
        first: u8,
    },

    /// Telegram returned a transport-level error status.
    #[snafu(display("Telegram transport returned status {status}"))]
    TransportStatus {
        /// Positive Telegram transport status.
        status: u32,
    },

    /// The framed stream failed.
    #[snafu(display("abridged MTProto framed stream failed"))]
    FramedIo {
        /// Underlying stream failure.
        source: io::Error,
    },

    /// Telegram closed the direct transport.
    #[snafu(display("Telegram closed the abridged MTProto transport"))]
    ConnectionClosed,
}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Self::FramedIo { source }
    }
}

/// Result returned by abridged transport operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Payload encoder/decoder used by Compio's reusable-buffer `Framed` API.
#[derive(Debug, Default)]
pub struct AbridgedCodec;

impl AbridgedCodec {
    /// Creates the payload codec.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Encodes one payload into a reusable buffer supplied by `Framed`.
    pub fn encode(&mut self, payload: &[u8], buffer: &mut Vec<u8>) -> Result<()> {
        validate_payload_length(payload.len())?;
        buffer.clear();
        buffer.reserve(payload.len());
        buffer.extend_from_slice(payload);
        Ok(())
    }

    fn decode_payload(payload: &[u8]) -> Result<Vec<u8>> {
        if payload.len() == 4 {
            let status = i32::from_le_bytes(
                payload[..4]
                    .try_into()
                    .expect("four-byte slice has the required length"),
            );
            if status < 0 {
                return TransportStatusSnafu {
                    status: status.unsigned_abs(),
                }
                .fail();
            }
        }
        Ok(payload.to_vec())
    }
}

impl Encoder<Vec<u8>, Vec<u8>> for AbridgedCodec {
    type Error = Error;

    fn encode(&mut self, payload: Vec<u8>, buffer: &mut Vec<u8>) -> Result<()> {
        self.encode(&payload, buffer)
    }
}

impl Decoder<Vec<u8>, Vec<u8>> for AbridgedCodec {
    type Error = Error;

    fn decode(&mut self, buffer: &Slice<Vec<u8>>) -> Result<Vec<u8>> {
        Self::decode_payload(buffer.as_init())
    }
}

#[derive(Debug, Default)]
struct AbridgedFramer {
    preamble_sent: bool,
}

impl AbridgedFramer {
    const fn new() -> Self {
        Self {
            preamble_sent: false,
        }
    }

    fn decode_header(first: u8, extended: Option<[u8; 3]>) -> Result<usize> {
        let words = match first {
            0..=126 => usize::from(first),
            LONG_HEADER => {
                let bytes = extended.context(MalformedHeaderSnafu { first })?;
                usize::try_from(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]))
                    .expect("24-bit word count fits usize")
            }
            _ => return MalformedHeaderSnafu { first }.fail(),
        };
        let length = words * 4;
        if length > MAX_PAYLOAD_BYTES {
            return PayloadTooLargeSnafu { length }.fail();
        }
        Ok(length)
    }
}

impl Framer<Vec<u8>> for AbridgedFramer {
    fn enclose(&mut self, buffer: &mut Vec<u8>) {
        let payload_len = buffer.len();
        let words = payload_len / 4;
        let header_len = if words < usize::from(LONG_HEADER) {
            1
        } else {
            4
        };
        let preamble_len = usize::from(!self.preamble_sent);
        let prefix_len = preamble_len + header_len;
        buffer.reserve(prefix_len);
        buffer.resize(payload_len + prefix_len, 0);
        buffer.copy_within(0..payload_len, prefix_len);
        let mut cursor = 0;
        if !self.preamble_sent {
            buffer[cursor] = PREAMBLE;
            self.preamble_sent = true;
            cursor += 1;
        }
        if words < usize::from(LONG_HEADER) {
            buffer[cursor] =
                u8::try_from(words).expect("short abridged word count fits in one byte");
        } else {
            let encoded = u32::try_from(words)
                .expect("validated abridged word count fits u32")
                .to_le_bytes();
            buffer[cursor..cursor + 4].copy_from_slice(&[
                LONG_HEADER,
                encoded[0],
                encoded[1],
                encoded[2],
            ]);
        }
    }

    fn extract(&mut self, buffer: &Slice<Vec<u8>>) -> io::Result<Option<Frame>> {
        let bytes = buffer.as_init();
        let Some(&first) = bytes.first() else {
            return Ok(None);
        };
        let header_len = if first < LONG_HEADER { 1 } else { 4 };
        if bytes.len() < header_len {
            return Ok(None);
        }
        let extended = (header_len == 4).then(|| [bytes[1], bytes[2], bytes[3]]);
        let payload_len = Self::decode_header(first, extended).map_err(io::Error::other)?;
        if bytes.len() < header_len + payload_len {
            return Ok(None);
        }
        Ok(Some(Frame::new(header_len, payload_len, 0)))
    }
}

type FramedTransport =
    SymmetricFramed<TcpStream, TcpStream, AbridgedCodec, AbridgedFramer, Vec<u8>, Vec<u8>>;

/// Direct TCP connection carrying abridged `MTProto` frames through Compio.
pub struct AbridgedConnection {
    framed: FramedTransport,
    queued_send: Option<Vec<u8>>,
    flushing_send: bool,
}

impl AbridgedConnection {
    /// Opens a direct Telegram TCP connection.
    pub async fn connect(endpoint: SocketAddr) -> Result<Self> {
        let stream = TcpStream::connect(endpoint)
            .await
            .context(ConnectSnafu { endpoint })?;
        let framed = SymmetricFramed::symmetric(AbridgedCodec::new(), AbridgedFramer::new())
            .with_duplex(stream)
            .with_buffer(Vec::with_capacity(4096), Vec::with_capacity(4096));
        Ok(Self {
            framed,
            queued_send: None,
            flushing_send: false,
        })
    }

    /// Sends one already-serialized `MTProto` envelope.
    pub async fn send(&mut self, payload: Vec<u8>) -> Result<()> {
        self.framed.send(payload).await
    }

    /// Receives one complete `MTProto` envelope.
    pub async fn receive(&mut self) -> Result<Vec<u8>> {
        self.framed.next().await.context(ConnectionClosedSnafu)?
    }

    pub(crate) fn queue_send(&mut self, payload: Vec<u8>) {
        debug_assert!(
            self.queued_send.is_none() && !self.flushing_send,
            "the connection driver queues only one frame at a time"
        );
        self.queued_send = Some(payload);
    }

    pub(crate) fn poll_send(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        if !self.flushing_send {
            ready!(Pin::new(&mut self.framed).poll_ready(cx))?;
            let payload = self
                .queued_send
                .take()
                .expect("the driver polls sending only with a queued frame");
            Pin::new(&mut self.framed).start_send(payload)?;
            self.flushing_send = true;
        }
        ready!(Pin::new(&mut self.framed).poll_flush(cx))?;
        self.flushing_send = false;
        Poll::Ready(Ok(()))
    }

    pub(crate) fn poll_receive(&mut self, cx: &mut Context<'_>) -> Poll<Result<Vec<u8>>> {
        match Pin::new(&mut self.framed).poll_next(cx) {
            Poll::Ready(Some(result)) => Poll::Ready(result),
            Poll::Ready(None) => Poll::Ready(ConnectionClosedSnafu.fail()),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Transport for AbridgedConnection {
    fn queue_send(mut self: Pin<&mut Self>, payload: Vec<u8>) {
        self.as_mut().get_mut().queue_send(payload);
    }

    fn poll_send(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.as_mut().get_mut().poll_send(cx)
    }

    fn poll_receive(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<Vec<u8>>> {
        self.as_mut().get_mut().poll_receive(cx)
    }
}

fn validate_payload_length(length: usize) -> Result<()> {
    if !length.is_multiple_of(4) {
        return UnalignedPayloadSnafu { length }.fail();
    }
    if length > MAX_PAYLOAD_BYTES {
        return PayloadTooLargeSnafu { length }.fail();
    }
    Ok(())
}

#[cfg(test)]
mod tests;
