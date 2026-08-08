use std::cell::RefCell;

use intuigram_app::{AttachmentId, Effect};
use snafu::ResultExt;

use super::super::super::{
    AttachmentPayload, PreparedRichMedia, ReadAttachmentSnafu, Result, backend, mime_type_for_path,
    record_media,
};
use super::State;

pub(in crate::application::actor_session) async fn attachment_payloads(
    effect: &Effect,
    state: &RefCell<State>,
) -> Result<Vec<(AttachmentId, AttachmentPayload)>> {
    let Effect::SendMessage { attachments, .. } = effect else {
        return Ok(Vec::new());
    };
    let payloads = attachments
        .iter()
        .filter_map(|id| {
            state
                .borrow()
                .attachments
                .payloads
                .get(id)
                .cloned()
                .map(|payload| (*id, payload))
        })
        .collect::<Vec<_>>();
    let mut prepared = Vec::with_capacity(payloads.len());
    for (id, payload) in payloads {
        let payload = match payload {
            AttachmentPayload::File { path, kind } => AttachmentPayload::PreparedFile {
                name: path.file_name().map_or_else(
                    || "attachment".to_owned(),
                    |name| name.to_string_lossy().into_owned(),
                ),
                mime_type: mime_type_for_path(&path),
                bytes: compio::fs::read(&path)
                    .await
                    .context(ReadAttachmentSnafu { path })?,
                kind,
            },
            payload => payload,
        };
        prepared.push((id, payload));
    }
    Ok(prepared)
}

pub(in crate::application::actor_session) async fn prepare_rich_media(
    effect: &Effect,
) -> Result<Option<PreparedRichMedia>> {
    let (path, kind, temporary) = match effect {
        Effect::SendRichMediaFile { path, kind, .. } => {
            (std::path::PathBuf::from(path), *kind, false)
        }
        Effect::RecordRichMedia {
            kind,
            seconds,
            device,
            ..
        } => (
            record_media(backend::upload_kind(*kind), *seconds, device).await?,
            *kind,
            true,
        ),
        _ => return Ok(None),
    };
    let result = compio::fs::read(&path)
        .await
        .context(ReadAttachmentSnafu { path: path.clone() });
    if temporary {
        let _ = compio::fs::remove_file(&path).await;
    }
    Ok(Some(PreparedRichMedia {
        name: path.file_name().map_or_else(
            || "media".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        ),
        mime_type: mime_type_for_path(&path),
        bytes: result?,
        kind,
    }))
}
