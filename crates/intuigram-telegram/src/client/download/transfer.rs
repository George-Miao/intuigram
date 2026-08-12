use futures_util::{StreamExt as _, TryStreamExt as _};

use super::super::*;
use super::location::DownloadLocation;

pub(super) const DOWNLOAD_PART_BYTES: i32 = 512 * 1024;
pub(super) const MAX_IN_FLIGHT_BYTES_PER_FILE: usize = 2 * 1024 * 1024;
pub(super) const DOWNLOAD_PART_CONCURRENCY: usize =
    MAX_IN_FLIGHT_BYTES_PER_FILE / DOWNLOAD_PART_BYTES as usize;
pub(super) const MAX_INLINE_PREVIEW_BYTES: usize = 8 * 1024 * 1024;
pub(super) const MAX_AVATAR_BYTES: usize = 2 * 1024 * 1024;

impl Client {
    pub(super) async fn download_location(
        &self,
        download: DownloadLocation,
    ) -> Result<DownloadedMedia> {
        self.download_known(download).await
    }

    pub(super) async fn media_connection(&self, dc_id: i32) -> Result<InvocationHandle> {
        self.media_sessions
            .as_ref()
            .context(MediaSessionUnavailableSnafu)?
            .connection(dc_id)
            .await
    }

    pub(super) async fn download_unknown(
        &self,
        dc_id: i32,
        location: tl::enums::InputFileLocation,
    ) -> Result<DownloadedMedia> {
        let mut bytes = Vec::new();
        while bytes.len() < MAX_AVATAR_BYTES {
            let offset = i64::try_from(bytes.len())
                .map_err(|_| Error::InvalidDownloadSize { size: i64::MAX })?;
            let part = self
                .fetch_file(dc_id, location.clone(), offset, DOWNLOAD_PART_BYTES)
                .await?;
            let complete = part.len() < DOWNLOAD_PART_BYTES as usize;
            bytes.extend_from_slice(&part);
            if complete {
                return Ok(DownloadedMedia {
                    name: "avatar.jpg".to_owned(),
                    mime_type: "image/jpeg".to_owned(),
                    bytes,
                });
            }
        }
        AvatarTooLargeSnafu {
            limit: MAX_AVATAR_BYTES,
        }
        .fail()
    }

    async fn download_known(&self, download: DownloadLocation) -> Result<DownloadedMedia> {
        let parts = download_parts(download.size)?;
        let mut parts = futures_util::stream::iter(parts)
            .map(|(index, offset, limit)| {
                let location = download.location.clone();
                let dc_id = download.dc_id;
                async move {
                    let bytes = self.fetch_file(dc_id, location, offset, limit).await?;
                    Ok((index, bytes))
                }
            })
            .buffer_unordered(DOWNLOAD_PART_CONCURRENCY)
            .try_collect::<Vec<_>>()
            .await?;
        parts.sort_unstable_by_key(|(index, _)| *index);
        let mut bytes = Vec::with_capacity(download.size);
        for (_, part) in parts {
            bytes.extend_from_slice(&part);
        }
        bytes.truncate(download.size);
        if bytes.len() != download.size {
            return IncompleteDownloadSnafu {
                expected: download.size,
                actual: bytes.len(),
            }
            .fail();
        }
        Ok(DownloadedMedia {
            name: download.name,
            mime_type: download.mime_type,
            bytes,
        })
    }

    async fn fetch_file(
        &self,
        initial_dc: i32,
        location: tl::enums::InputFileLocation,
        offset: i64,
        limit: i32,
    ) -> Result<Vec<u8>> {
        let mut dc_id = initial_dc;
        for _ in 0..3 {
            let connection = self.media_connection(dc_id).await?;
            match connection
                .invoke(&tl::functions::upload::GetFile {
                    precise: false,
                    cdn_supported: true,
                    location: location.clone(),
                    offset,
                    limit,
                })
                .await
            {
                Ok(tl::enums::upload::File::File(file)) => return Ok(file.bytes),
                Ok(tl::enums::upload::File::CdnRedirect(redirect)) => {
                    match self
                        .download_cdn_part(dc_id, &redirect, offset, limit)
                        .await
                    {
                        Ok(bytes) => return Ok(bytes),
                        Err(error) if super::cdn::token_invalid(&error) => {
                            return self.fetch_master_part(dc_id, location, offset, limit).await;
                        }
                        Err(error) => return Err(error),
                    }
                }
                Err(error) => {
                    let Some(target) = rpc_migration_dc(&error, "FILE_MIGRATE_") else {
                        return Err(Error::Invoke { source: error });
                    };
                    dc_id = target;
                }
            }
        }
        FileMigrationLoopSnafu { dc_id }.fail()
    }

    async fn fetch_master_part(
        &self,
        dc_id: i32,
        location: tl::enums::InputFileLocation,
        offset: i64,
        limit: i32,
    ) -> Result<Vec<u8>> {
        match self
            .media_connection(dc_id)
            .await?
            .invoke(&tl::functions::upload::GetFile {
                precise: false,
                cdn_supported: false,
                location,
                offset,
                limit,
            })
            .await
            .context(InvokeSnafu)?
        {
            tl::enums::upload::File::File(file) => Ok(file.bytes),
            tl::enums::upload::File::CdnRedirect(_) => CdnReuploadLoopSnafu.fail(),
        }
    }
}

pub(super) fn download_parts(size: usize) -> Result<Vec<(usize, i64, i32)>> {
    let part_bytes = DOWNLOAD_PART_BYTES as usize;
    (0..size.div_ceil(part_bytes))
        .map(|index| {
            let offset = index.saturating_mul(part_bytes);
            Ok((
                index,
                i64::try_from(offset).map_err(|_| Error::InvalidDownloadSize { size: i64::MAX })?,
                DOWNLOAD_PART_BYTES,
            ))
        })
        .collect()
}
