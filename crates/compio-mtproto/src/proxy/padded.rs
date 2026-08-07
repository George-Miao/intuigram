use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use compio::buf::{IoBuf, Slice};
use compio::io::framed::SymmetricFramed;
use compio::io::framed::frame::{Frame, Framer};
use futures_util::{Sink, Stream};
use snafu::Snafu;

use super::mtproxy::{Reader, Writer};
use crate::{AbridgedCodec, Transport, TransportError};

const MAX_PAYLOAD_BYTES: usize = 0x00ff_ffff * 4;

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("padded-intermediate frame length {length} exceeds the protocol limit"))]
    PayloadTooLarge { length: usize },
}

#[derive(Default)]
struct PaddedFramer;

impl Framer<Vec<u8>> for PaddedFramer {
    fn enclose(&mut self, buffer: &mut Vec<u8>) {
        let payload = buffer.len();
        let padding = getrandom::u32().map_or(0, |value| value as usize % 16);
        let total = payload + padding;
        buffer.reserve(4 + padding);
        buffer.resize(total + 4, 0);
        buffer.copy_within(0..payload, 4);
        let _ = getrandom::fill(&mut buffer[4 + payload..]);
        buffer[..4].copy_from_slice(
            &u32::try_from(total)
                .expect("MTProto transport length is bounded below u32")
                .to_le_bytes(),
        );
    }

    fn extract(&mut self, buffer: &Slice<Vec<u8>>) -> io::Result<Option<Frame>> {
        let bytes = buffer.as_init();
        if bytes.len() < 4 {
            return Ok(None);
        }
        let length = u32::from_le_bytes(
            bytes[..4]
                .try_into()
                .expect("four checked bytes fit a transport length"),
        ) as usize;
        if length > MAX_PAYLOAD_BYTES + 15 {
            return Err(io::Error::other(Error::PayloadTooLarge { length }));
        }
        if bytes.len() < length + 4 {
            return Ok(None);
        }
        Ok(Some(Frame::new(4, length, 0)))
    }
}

type Framed = SymmetricFramed<Reader, Writer, AbridgedCodec, PaddedFramer, Vec<u8>, Vec<u8>>;

pub(crate) struct Connection {
    framed: Framed,
    queued_send: Option<Vec<u8>>,
    flushing_send: bool,
}

impl Connection {
    pub(crate) fn new(reader: Reader, writer: Writer) -> Self {
        let framed = SymmetricFramed::symmetric(AbridgedCodec::new(), PaddedFramer)
            .with_reader(reader)
            .with_writer(writer)
            .with_buffer(Vec::with_capacity(4096), Vec::with_capacity(4096));
        Self {
            framed,
            queued_send: None,
            flushing_send: false,
        }
    }
}

impl Transport for Connection {
    fn queue_send(mut self: Pin<&mut Self>, payload: Vec<u8>) {
        debug_assert!(self.queued_send.is_none() && !self.flushing_send);
        self.queued_send = Some(payload);
    }

    fn poll_send(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), TransportError>> {
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

    fn poll_receive(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Vec<u8>, TransportError>> {
        match Pin::new(&mut self.framed).poll_next(cx) {
            Poll::Ready(Some(result)) => Poll::Ready(result),
            Poll::Ready(None) => Poll::Ready(Err(TransportError::ConnectionClosed)),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padded_frames_include_total_length_and_at_most_fifteen_padding_bytes() {
        let mut framer = PaddedFramer;
        let payload = vec![7; 32];
        let mut frame = payload.clone();
        framer.enclose(&mut frame);
        let total = u32::from_le_bytes(frame[..4].try_into().expect("length header")) as usize;
        assert!((payload.len()..=payload.len() + 15).contains(&total));
        assert_eq!(&frame[4..4 + payload.len()], payload.as_slice());
    }
}
