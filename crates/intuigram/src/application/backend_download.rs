use compio::runtime::ResumeUnwind;

use super::*;

impl Backend {
    pub(super) async fn load_media_preview(
        &mut self,
        chat: ChatId,
        message: MessageId,
    ) -> Result<Option<InlineImage>> {
        let media = self
            .client
            .download_media(chat, message)
            .await
            .context(TelegramSnafu)?;
        Ok(compio::runtime::spawn_blocking(move || {
            intuigram_media::decode_preview(&media.bytes).ok()
        })
        .await
        .resume_unwind()
        .expect("an awaited blocking media-preview task cannot be cancelled"))
    }

    pub(super) async fn download_media(
        &mut self,
        chat: ChatId,
        message: MessageId,
        destination: Option<String>,
    ) -> Result<DownloadView> {
        let media = self
            .client
            .download_media(chat, message)
            .await
            .context(TelegramSnafu)?;
        let name = media.name;
        let mime_type = media.mime_type;
        let bytes = media.bytes;
        let (bytes, preview) = compio::runtime::spawn_blocking(move || {
            let preview = intuigram_media::decode_preview(&bytes).ok();
            (bytes, preview)
        })
        .await
        .resume_unwind()
        .expect("an awaited blocking media-preview task cannot be cancelled");
        let path = match destination {
            Some(destination) => self.downloads.save_to(destination, bytes).await,
            None => self.downloads.save(&name, bytes).await,
        }
        .context(SaveDownloadSnafu)?;
        let reveal_only = intuigram_media::open_disposition(&path, Some(&mime_type))
            == intuigram_media::OpenDisposition::RevealWithLaunchWarning;
        let id = self.downloaded.register(path.clone());
        Ok(DownloadView {
            chat,
            id,
            path: path.display().to_string(),
            reveal_only,
            message,
            preview,
        })
    }
}
