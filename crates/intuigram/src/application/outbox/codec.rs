use snafu::{ResultExt, Snafu};

use super::model::PreparedCommand;

const MAGIC: &[u8; 4] = b"ICMD";
const V1: u8 = 1;

#[derive(Debug, Snafu)]
pub(in crate::application) enum Error {
    #[snafu(display("prepared Outbox command ended before its header was decoded"))]
    Truncated,

    #[snafu(display("prepared Outbox command has an invalid header"))]
    InvalidHeader,

    #[snafu(display("prepared Outbox command version {version} is unsupported"))]
    UnsupportedVersion { version: u8 },

    #[snafu(display("prepared Outbox command could not be encoded"))]
    Encode { source: serde_json::Error },

    #[snafu(display("prepared Outbox command content is corrupt"))]
    Corrupt { source: serde_json::Error },
}

pub(super) type Result<T> = std::result::Result<T, Error>;

pub(super) fn encode(command: &PreparedCommand) -> Result<Vec<u8>> {
    let body = serde_json::to_vec(command).context(EncodeSnafu)?;
    let mut bytes = Vec::with_capacity(MAGIC.len() + 1 + body.len());
    bytes.extend_from_slice(MAGIC);
    bytes.push(V1);
    bytes.extend_from_slice(&body);
    Ok(bytes)
}

pub(super) fn decode(bytes: &[u8]) -> Result<PreparedCommand> {
    if bytes.len() < MAGIC.len() + 1 {
        return Err(Error::Truncated);
    }
    if &bytes[..MAGIC.len()] != MAGIC {
        return Err(Error::InvalidHeader);
    }
    match bytes[MAGIC.len()] {
        V1 => serde_json::from_slice(&bytes[MAGIC.len() + 1..]).context(CorruptSnafu),
        version => Err(Error::UnsupportedVersion { version }),
    }
}
