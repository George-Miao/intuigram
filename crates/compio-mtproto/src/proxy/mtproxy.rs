use std::io;

use aes::Aes256;
use aes::cipher::{KeyIvInit, StreamCipher};
use compio::buf::{BufResult, IoBuf, IoBufMut};
use compio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use compio::net::TcpStream;
use ctr::Ctr128BE;
use sha2::{Digest, Sha256};

use super::types::{MtProxySecret, MtProxyTransport};

type Cipher = Ctr128BE<Aes256>;

pub(crate) struct Reader {
    stream: TcpStream,
    cipher: Cipher,
}

pub(crate) struct Writer {
    stream: TcpStream,
    cipher: Cipher,
}

pub(crate) async fn initialize(
    mut stream: TcpStream,
    dc_id: i32,
    secret: &MtProxySecret,
) -> io::Result<(Reader, Writer)> {
    let mut random = [0_u8; 64];
    loop {
        getrandom::fill(&mut random).map_err(|error| io::Error::other(error.to_string()))?;
        if valid_header(&random) {
            break;
        }
    }
    random[56..60].copy_from_slice(match secret.transport {
        MtProxyTransport::Abridged => &[0xef; 4],
        MtProxyTransport::PaddedIntermediate => &[0xdd; 4],
    });
    let dc_id = i16::try_from(dc_id)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "MTProxy DC id is invalid"))?;
    random[60..62].copy_from_slice(&dc_id.to_le_bytes());

    let mut reversed = random[8..56].to_vec();
    reversed.reverse();
    let mut encrypt = cipher(&random[8..40], &random[40..56], secret);
    let decrypt = cipher(&reversed[..32], &reversed[32..48], secret);
    let mut encrypted = random;
    encrypt.apply_keystream(&mut encrypted);
    random[56..64].copy_from_slice(&encrypted[56..64]);
    let BufResult(result, _) = stream.write_all(random.to_vec()).await;
    result?;
    Ok((
        Reader {
            stream: stream.clone(),
            cipher: decrypt,
        },
        Writer {
            stream,
            cipher: encrypt,
        },
    ))
}

fn cipher(key: &[u8], iv: &[u8], secret: &MtProxySecret) -> Cipher {
    let mut digest = Sha256::new();
    digest.update(key);
    digest.update(secret.bytes);
    let key = digest.finalize();
    Cipher::new_from_slices(&key, iv).expect("SHA-256 key and MTProxy IV have fixed lengths")
}

fn valid_header(header: &[u8; 64]) -> bool {
    !matches!(
        &header[..4],
        b"GET " | b"POST" | b"HEAD" | b"OPTI" | &[0x16, 0x03, 0x01, 0x02]
    ) && header[..4] != [0xdd; 4]
        && header[..4] != [0xee; 4]
        && header[4..8] != [0; 4]
        && header[0] != 0xef
}

impl AsyncRead for Reader {
    async fn read<B: IoBufMut>(&mut self, buffer: B) -> BufResult<usize, B> {
        let BufResult(result, mut buffer) = self.stream.read(buffer).await;
        if let Ok(length) = result {
            self.cipher
                .apply_keystream(&mut buffer.as_mut_slice()[..length]);
        }
        BufResult(result, buffer)
    }
}

impl AsyncWrite for Writer {
    async fn write<B: IoBuf>(&mut self, buffer: B) -> BufResult<usize, B> {
        let length = buffer.buf_len();
        let mut encrypted = buffer.as_init().to_vec();
        self.cipher.apply_keystream(&mut encrypted);
        let BufResult(result, _) = self.stream.write_all(encrypted).await;
        BufResult(result.map(|()| length), buffer)
    }

    async fn flush(&mut self) -> io::Result<()> {
        self.stream.flush().await
    }

    async fn shutdown(&mut self) -> io::Result<()> {
        self.stream.shutdown().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_header_rejects_plain_protocol_signatures() {
        let mut header = [7_u8; 64];
        assert!(valid_header(&header));
        header[..4].copy_from_slice(b"GET ");
        assert!(!valid_header(&header));
        header[..4].copy_from_slice(b"OPTI");
        assert!(!valid_header(&header));
    }
}
