use std::io::Write;

use intuigram_app::{AdapterEvent, ChatId, DownloadId, DownloadView, MediaKind, MessageId};
use snafu::ResultExt;

use super::TestSystem;
pub(super) const ONE_PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c,
    0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0x00,
    0x01, 0x05, 0x01, 0x01, 0x27, 0x18, 0xe3, 0x66, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];
use crate::error::{Result, WriteMediaSnafu};

impl TestSystem {
    pub(super) fn download_media_effect(
        &mut self,
        chat: ChatId,
        message: MessageId,
        destination: Option<String>,
    ) -> Result<()> {
        let view = self.application.view();
        let media = view
            .messages
            .iter()
            .find(|candidate| candidate.id == message)
            .and_then(|message| message.details.media.as_ref());
        let name = media
            .map(|media| media.title.clone())
            .unwrap_or_else(|| "download".to_owned());
        let image = media.is_some_and(|media| {
            matches!(
                media.kind,
                MediaKind::Photo | MediaKind::Animation | MediaKind::Sticker
            )
        });
        let bytes = if image {
            ONE_PIXEL_PNG
        } else {
            b"behavior media"
        };
        let preview = image
            .then(|| intuigram_media::decode_preview(bytes).ok())
            .flatten();
        let path = destination.map_or_else(|| self.download_root.join(name), Into::into);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context(WriteMediaSnafu { path: path.clone() })?;
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .context(WriteMediaSnafu { path: path.clone() })?;
        file.write_all(bytes)
            .context(WriteMediaSnafu { path: path.clone() })?;
        self.next_download_id = self.next_download_id.saturating_add(1);
        let id = DownloadId(self.next_download_id);
        let reveal_only = intuigram_media::open_disposition(&path, None)
            == intuigram_media::OpenDisposition::RevealWithLaunchWarning;
        self.downloaded_paths.push(path.clone());
        self.application
            .handle_adapter(AdapterEvent::DownloadReady {
                chat,
                download: DownloadView {
                    chat,
                    id,
                    path: path.display().to_string(),
                    reveal_only,
                    message,
                    preview,
                },
            });
        Ok(())
    }
}
