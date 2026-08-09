use intuigram_store::OutboxMedia;
use intuigram_telegram::MediaLibraryEntry;

use super::super::super::{AttachmentPayload, PreparedRichMedia};
use super::super::model::shared::{MediaPosition, PreparedAttachment, PreparedMedia};
use super::{Error, Result, conversion};

pub(super) struct PreparedInputs {
    attachments: Vec<(intuigram_app::AttachmentId, AttachmentPayload)>,
    rich_media: Option<PreparedRichMedia>,
    library: Option<MediaLibraryEntry>,
}

impl PreparedInputs {
    pub(super) const fn new(
        attachments: Vec<(intuigram_app::AttachmentId, AttachmentPayload)>,
        rich_media: Option<PreparedRichMedia>,
        library: Option<MediaLibraryEntry>,
    ) -> Self {
        Self {
            attachments,
            rich_media,
            library,
        }
    }

    pub(super) fn attachments(
        &mut self,
        ids: &[intuigram_app::AttachmentId],
    ) -> Result<(Vec<PreparedAttachment>, Vec<OutboxMedia>)> {
        let mut command = Vec::with_capacity(ids.len());
        let mut media = Vec::with_capacity(ids.len());
        for id in ids {
            let position = u32::try_from(media.len()).map_err(|_| Error::NumericOverflow)?;
            let payload = self
                .attachments
                .iter()
                .find(|(candidate, _)| candidate == id)
                .map(|(_, payload)| payload)
                .ok_or(Error::Incomplete {
                    reason: "a selected attachment was not prepared",
                })?;
            let (kind, item) = outbox_media(payload)?;
            command.push(PreparedAttachment::new(MediaPosition(position), kind));
            media.push(item);
        }
        Ok((command, media))
    }

    pub(super) fn rich_media(&mut self) -> Result<(PreparedMedia, Vec<OutboxMedia>)> {
        let prepared = self.rich_media.take().ok_or(Error::Incomplete {
            reason: "a file or recording was not prepared",
        })?;
        let command = PreparedMedia::new(MediaPosition(0), conversion::upload_kind(prepared.kind));
        let media = OutboxMedia::new(prepared.name, prepared.mime_type, prepared.bytes);
        Ok((command, vec![media]))
    }

    pub(super) fn library(&mut self) -> Result<MediaLibraryEntry> {
        self.library.take().ok_or(Error::Incomplete {
            reason: "the selected Telegram media item is unavailable",
        })
    }
}

fn outbox_media(
    payload: &AttachmentPayload,
) -> Result<(super::super::model::shared::AttachmentKind, OutboxMedia)> {
    match payload {
        AttachmentPayload::Image { mime_type, bytes } => Ok((
            super::super::model::shared::AttachmentKind::Photo,
            OutboxMedia::new("clipboard.png".to_owned(), mime_type.clone(), bytes.clone()),
        )),
        AttachmentPayload::PreparedFile {
            name,
            mime_type,
            bytes,
            kind,
        } => Ok((
            conversion::attachment_kind(*kind),
            OutboxMedia::new(name.clone(), mime_type.clone(), bytes.clone()),
        )),
        AttachmentPayload::File { .. } => Err(Error::Incomplete {
            reason: "an attachment path reached admission before its bytes were read",
        }),
    }
}
