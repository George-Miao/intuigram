use std::io::Write;

use intuigram_app::{AdapterEvent, ChatId, DownloadId, DownloadView, MediaKind, MessageId};
use snafu::ResultExt;

use super::TestSystem;
use super::effects::ONE_PIXEL_PNG;
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
