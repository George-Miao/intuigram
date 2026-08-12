use aes::Aes256;
use aes::cipher::{KeyIvInit as _, StreamCipher as _};
use ctr::Ctr128BE;
use sha2::{Digest as _, Sha256};

use super::super::*;

impl Client {
    pub(super) async fn download_cdn_part(
        &self,
        master_dc: i32,
        redirect: &tl::types::upload::FileCdnRedirect,
        offset: i64,
        limit: i32,
    ) -> Result<Vec<u8>> {
        let sessions = self
            .media_sessions
            .as_ref()
            .context(MediaSessionUnavailableSnafu)?;
        let mut reuploaded = false;
        let encrypted = loop {
            match sessions
                .cdn_file(redirect.dc_id, redirect.file_token.clone(), offset, limit)
                .await?
            {
                tl::enums::upload::CdnFile::File(file) => break file.bytes,
                tl::enums::upload::CdnFile::ReuploadNeeded(request) if !reuploaded => {
                    reuploaded = true;
                    self.media_connection(master_dc)
                        .await?
                        .invoke(&tl::functions::upload::ReuploadCdnFile {
                            file_token: redirect.file_token.clone(),
                            request_token: request.request_token,
                        })
                        .await
                        .context(InvokeSnafu)?;
                }
                tl::enums::upload::CdnFile::ReuploadNeeded(_) => {
                    return CdnReuploadLoopSnafu.fail();
                }
            }
        };
        let bytes = decrypt(
            &redirect.encryption_key,
            &redirect.encryption_iv,
            offset,
            encrypted,
        )?;
        let mut hashes = redirect.file_hashes.clone();
        if !hashes_cover(offset, bytes.len(), &hashes) {
            hashes.extend(
                self.media_connection(master_dc)
                    .await?
                    .invoke(&tl::functions::upload::GetCdnFileHashes {
                        file_token: redirect.file_token.clone(),
                        offset,
                    })
                    .await
                    .context(InvokeSnafu)?,
            );
        }
        verify(offset, &bytes, &hashes)?;
        Ok(bytes)
    }
}

fn decrypt(key: &[u8], iv: &[u8], offset: i64, mut bytes: Vec<u8>) -> Result<Vec<u8>> {
    if offset < 0 || offset % 16 != 0 {
        return InvalidCdnEncryptionMaterialSnafu.fail();
    }
    let block = u32::try_from(offset / 16).map_err(|_| Error::InvalidCdnEncryptionMaterial)?;
    let key: [u8; 32] = key
        .try_into()
        .map_err(|_| Error::InvalidCdnEncryptionMaterial)?;
    let mut iv: [u8; 16] = iv
        .try_into()
        .map_err(|_| Error::InvalidCdnEncryptionMaterial)?;
    iv[12..].copy_from_slice(&block.to_be_bytes());
    let mut cipher = Ctr128BE::<Aes256>::new(&key.into(), &iv.into());
    cipher.apply_keystream(&mut bytes);
    Ok(bytes)
}

fn hashes_cover(offset: i64, size: usize, hashes: &[tl::enums::FileHash]) -> bool {
    let mut cursor = offset;
    let Some(end) = offset.checked_add(i64::try_from(size).unwrap_or(i64::MAX)) else {
        return false;
    };
    while cursor < end {
        let Some(hash) = hashes.iter().find_map(|hash| {
            let tl::enums::FileHash::Hash(hash) = hash;
            (hash.offset == cursor).then_some(hash)
        }) else {
            return false;
        };
        let limit = i64::from(hash.limit);
        if limit <= 0 {
            return false;
        }
        if cursor.saturating_add(limit) > end {
            return false;
        }
        cursor += limit;
    }
    true
}

fn verify(offset: i64, bytes: &[u8], hashes: &[tl::enums::FileHash]) -> Result<()> {
    let mut position = 0usize;
    while position < bytes.len() {
        let absolute = offset
            .checked_add(i64::try_from(position).unwrap_or(i64::MAX))
            .context(CdnHashUnavailableSnafu { offset })?;
        let hash = hashes
            .iter()
            .find_map(|hash| {
                let tl::enums::FileHash::Hash(hash) = hash;
                (hash.offset == absolute).then_some(hash)
            })
            .context(CdnHashUnavailableSnafu { offset: absolute })?;
        let limit = usize::try_from(hash.limit)
            .ok()
            .filter(|limit| *limit > 0)
            .context(CdnHashUnavailableSnafu { offset: absolute })?;
        let end = position
            .checked_add(limit)
            .filter(|end| *end <= bytes.len())
            .context(CdnHashUnavailableSnafu { offset: absolute })?;
        let digest: [u8; 32] = Sha256::digest(&bytes[position..end]).into();
        if digest.as_slice() != hash.hash.as_slice() {
            return CdnHashMismatchSnafu { offset: absolute }.fail();
        }
        position = end;
    }
    Ok(())
}

pub(super) fn token_invalid(error: &Error) -> bool {
    matches!(
        error,
        Error::Invoke {
            source: InvocationError::Rpc { message, .. },
        } if message == "FILE_TOKEN_INVALID"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctr_decryption_uses_the_part_offset_in_the_iv() {
        let key = [7_u8; 32];
        let iv = [9_u8; 16];
        let plain = b"cdn payload with details".to_vec();
        let encrypted = decrypt(&key, &iv, 4096, plain.clone()).expect("encryption should work");

        assert_ne!(encrypted, plain);
        assert_eq!(
            decrypt(&key, &iv, 4096, encrypted).expect("decryption should work"),
            plain
        );
    }

    #[test]
    fn every_decrypted_range_requires_a_matching_hash() {
        let bytes = b"trusted bytes";
        let hashes = vec![
            tl::types::FileHash {
                offset: 4096,
                limit: i32::try_from(bytes.len()).expect("fixture length fits"),
                hash: Sha256::digest(bytes).to_vec(),
            }
            .into(),
        ];

        verify(4096, bytes, &hashes).expect("matching hash should verify");
        assert!(matches!(
            verify(4096, b"tampered data", &hashes),
            Err(Error::CdnHashMismatch { .. })
        ));
    }
}
