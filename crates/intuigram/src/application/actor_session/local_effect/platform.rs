use std::cell::RefCell;
use std::io::{self, Write};
use std::path::PathBuf;

use compio::runtime::ResumeUnwind;
use intuigram_app::{AdapterEvent, AttachmentKind, AttachmentView, Effect};
use snafu::ResultExt;

use super::super::super::{
    AttachmentPayload, NotificationSnafu, ReadAttachmentSnafu, Result, mime_type_for_path,
};
use super::State;

pub(super) async fn execute(
    effect: Effect,
    state: &RefCell<State>,
) -> Result<Option<AdapterEvent>> {
    let event = match effect {
        Effect::Notify { .. } => {
            compio::runtime::spawn_blocking(|| -> io::Result<()> {
                let mut stderr = io::stderr().lock();
                stderr.write_all(b"\x07")?;
                stderr.flush()
            })
            .await
            .resume_unwind()
            .expect("an awaited terminal-bell task cannot be cancelled")
            .context(NotificationSnafu)?;
            None
        }
        Effect::OpenExternalLink { url } => Some(
            match intuigram_media::PlatformLauncher.open_url(&url).await {
                Ok(()) => AdapterEvent::OperationCompleted(format!("Opened {url}")),
                Err(error) => AdapterEvent::OperationFailed(error.to_string()),
            },
        ),
        Effect::ReadClipboard { chat, thread_root } => Some(
            read_clipboard(state, chat, thread_root)
                .await
                .unwrap_or_else(|error| AdapterEvent::OperationFailed(error.to_string())),
        ),
        Effect::SelectAttachment {
            chat,
            thread_root,
            path,
        } => Some(select_attachment(state, chat, thread_root, path).await),
        Effect::OpenDownload { download, reveal } => {
            let path = state.borrow().downloaded.paths.get(&download).cloned();
            Some(match path {
                Some(path) => open_download(path, reveal).await,
                None => AdapterEvent::OperationFailed(format!(
                    "completed download {} is no longer available",
                    download.0
                )),
            })
        }
        _ => unreachable!("only independent local effects reach the platform executor"),
    };
    Ok(event)
}

async fn read_clipboard(
    state: &RefCell<State>,
    chat: intuigram_app::ChatId,
    thread_root: Option<intuigram_app::MessageId>,
) -> rich_clipboard::Result<AdapterEvent> {
    let content = rich_clipboard::read().await?;
    let (text, attachments) = match content {
        rich_clipboard::ClipboardContent::Text(text) => (Some(text), Vec::new()),
        rich_clipboard::ClipboardContent::Image { mime_type, bytes } => {
            let id = state
                .borrow_mut()
                .attachments
                .register(AttachmentPayload::Image { mime_type, bytes });
            (
                None,
                vec![AttachmentView {
                    id,
                    kind: AttachmentKind::Photo,
                    name: "clipboard.png".to_owned(),
                }],
            )
        }
        rich_clipboard::ClipboardContent::Files(paths) => {
            let attachments = paths
                .into_iter()
                .map(|path| register_file(state, path))
                .collect();
            (None, attachments)
        }
    };
    Ok(AdapterEvent::ClipboardReady {
        chat,
        thread_root,
        text,
        attachments,
    })
}

async fn select_attachment(
    state: &RefCell<State>,
    chat: intuigram_app::ChatId,
    thread_root: Option<intuigram_app::MessageId>,
    path: String,
) -> AdapterEvent {
    let path = PathBuf::from(path);
    match compio::fs::metadata(&path)
        .await
        .context(ReadAttachmentSnafu { path: path.clone() })
    {
        Ok(metadata) if metadata.is_file() => AdapterEvent::ClipboardReady {
            chat,
            thread_root,
            text: None,
            attachments: vec![register_file(state, path)],
        },
        Ok(_) => {
            AdapterEvent::OperationFailed("Attachment path must identify a regular file".to_owned())
        }
        Err(error) => AdapterEvent::OperationFailed(error.to_string()),
    }
}

fn register_file(state: &RefCell<State>, path: PathBuf) -> AttachmentView {
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
    let id = state
        .borrow_mut()
        .attachments
        .register(AttachmentPayload::File { path, kind });
    AttachmentView { id, kind, name }
}

async fn open_download(path: PathBuf, reveal: bool) -> AdapterEvent {
    let result = if reveal {
        intuigram_media::PlatformLauncher.reveal_file(&path).await
    } else {
        intuigram_media::PlatformLauncher.open_file(&path).await
    };
    match result {
        Ok(()) => AdapterEvent::OperationCompleted(if reveal {
            format!("Revealed {}", path.display())
        } else {
            format!("Opened {}", path.display())
        }),
        Err(error) => AdapterEvent::OperationFailed(error.to_string()),
    }
}
