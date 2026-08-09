use super::*;

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
