use snafu::Snafu;

use super::{OutboxPayload, OutboxPayloadV1};

const MAGIC: &[u8; 4] = b"IOBX";
const V1: u8 = 1;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Outbox payload has an invalid header"))]
    InvalidHeader,

    #[snafu(display("Outbox payload version {version} is unsupported"))]
    UnsupportedVersion { version: u8 },

    #[snafu(display("Outbox payload ended before all fields were decoded"))]
    Truncated,

    #[snafu(display("Outbox payload contains trailing bytes"))]
    TrailingBytes,

    #[snafu(display("Outbox payload content is too large"))]
    ContentTooLarge,

    #[snafu(display("Outbox payload has an invalid optional-field tag {tag}"))]
    InvalidOptionalTag { tag: u8 },
}

pub(super) type Result<T> = std::result::Result<T, Error>;

pub(super) fn encode(payload: &OutboxPayload) -> Result<Vec<u8>> {
    match payload {
        OutboxPayload::V1(payload) => encode_v1(payload),
    }
}

fn encode_v1(payload: &OutboxPayloadV1) -> Result<Vec<u8>> {
    let content_length =
        u32::try_from(payload.content.len()).map_err(|_| Error::ContentTooLarge)?;
    let mut bytes = Vec::with_capacity(51 + payload.content.len());
    bytes.extend_from_slice(MAGIC);
    bytes.push(V1);
    bytes.extend_from_slice(&payload.chat_id.to_le_bytes());
    push_optional(&mut bytes, payload.thread_root);
    push_optional(&mut bytes, payload.saved_peer);
    push_optional(&mut bytes, payload.local_message_id);
    bytes.extend_from_slice(&payload.random_id.to_le_bytes());
    bytes.extend_from_slice(&content_length.to_le_bytes());
    bytes.extend_from_slice(&payload.content);
    Ok(bytes)
}

fn push_optional(bytes: &mut Vec<u8>, value: Option<i64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

pub(super) fn decode(bytes: &[u8]) -> Result<OutboxPayload> {
    let mut cursor = Cursor::new(bytes);
    if cursor.take(4)? != MAGIC {
        return Err(Error::InvalidHeader);
    }
    let version = cursor.byte()?;
    let payload = match version {
        V1 => OutboxPayload::V1(OutboxPayloadV1 {
            chat_id: cursor.i64()?,
            thread_root: cursor.optional_i64()?,
            saved_peer: cursor.optional_i64()?,
            local_message_id: cursor.optional_i64()?,
            random_id: cursor.i64()?,
            content: {
                let length = usize::try_from(cursor.u32()?).map_err(|_| Error::ContentTooLarge)?;
                cursor.take(length)?.to_vec()
            },
        }),
        version => return Err(Error::UnsupportedVersion { version }),
    };
    if cursor.remaining() != 0 {
        return Err(Error::TrailingBytes);
    }
    Ok(payload)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self.position.checked_add(length).ok_or(Error::Truncated)?;
        let value = self.bytes.get(self.position..end).ok_or(Error::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?.try_into().map_err(|_| Error::Truncated)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn i64(&mut self) -> Result<i64> {
        let bytes = self.take(8)?.try_into().map_err(|_| Error::Truncated)?;
        Ok(i64::from_le_bytes(bytes))
    }

    fn optional_i64(&mut self) -> Result<Option<i64>> {
        match self.byte()? {
            0 => Ok(None),
            1 => self.i64().map(Some),
            tag => Err(Error::InvalidOptionalTag { tag }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_round_trips_without_adapter_types() {
        let payload = OutboxPayload::V1(OutboxPayloadV1 {
            chat_id: -1007,
            thread_root: Some(8),
            saved_peer: None,
            local_message_id: Some(-9),
            random_id: 10,
            content: vec![0, 1, 255],
        });

        assert_eq!(
            decode(&encode(&payload).expect("payload should encode"))
                .expect("payload should decode"),
            payload
        );
    }
}
