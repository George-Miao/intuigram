use super::*;
impl Backend {
    pub(super) async fn register_attachment_file(&mut self, path: PathBuf) -> AttachmentView {
        let name = path.file_name().map_or_else(
            || "attachment".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let mime_type = mime_type_for_path(&path);
        let kind = if mime_type.starts_with("image/") {
            AttachmentKind::Photo
        } else if mime_type.starts_with("video/") {
            AttachmentKind::Video
        } else {
            AttachmentKind::File
        };
        let preview = if kind == AttachmentKind::Photo {
            compio::fs::read(&path)
                .await
                .ok()
                .and_then(|bytes| intuigram_media::decode_preview(&bytes).ok())
        } else {
            None
        };
        let id = self
            .attachment_store()
            .register(AttachmentPayload::File { path, kind });
        AttachmentView {
            id,
            kind,
            name,
            preview,
            active: false,
        }
    }
}

pub(super) fn prepared_upload(payload: &AttachmentPayload) -> Result<intuigram_telegram::Upload> {
    match payload {
        AttachmentPayload::Image { mime_type, bytes } => Ok(intuigram_telegram::Upload {
            name: "clipboard.png".to_owned(),
            mime_type: mime_type.clone(),
            bytes: bytes.clone(),
            kind: intuigram_telegram::UploadKind::Photo,
        }),
        AttachmentPayload::File { path, .. } => {
            Err(Error::UnpreparedAttachment { path: path.clone() })
        }
        AttachmentPayload::PreparedFile {
            name,
            mime_type,
            bytes,
            kind,
        } => Ok(intuigram_telegram::Upload {
            name: name.clone(),
            mime_type: mime_type.clone(),
            bytes: bytes.clone(),
            kind: match kind {
                AttachmentKind::Photo => intuigram_telegram::UploadKind::Photo,
                AttachmentKind::Video => intuigram_telegram::UploadKind::Video,
                AttachmentKind::File => intuigram_telegram::UploadKind::File,
            },
        }),
    }
}
