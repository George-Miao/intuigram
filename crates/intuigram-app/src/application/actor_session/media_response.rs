use intuigram_lib::{AdapterEvent, OfflineMediaFailure, OfflineMediaTarget};
use snafu::ResultExt;

use super::super::{AdapterBatch, Error, TelegramSnafu};
use super::actor::{ActorResponse, TelegramState};
use super::cancellation::{ActorCancellation, until_cancelled};

pub(super) async fn avatar(
    state: &mut TelegramState,
    cancellation: &ActorCancellation,
    avatar: intuigram_lib::AvatarRef,
) -> ActorResponse {
    let result = until_cancelled(
        async {
            state
                .backend
                .client
                .download_avatar(avatar)
                .await
                .context(TelegramSnafu)
        },
        cancellation,
    )
    .await;
    match result {
        Ok(media) => ActorResponse::Avatar {
            avatar,
            media: media.map(Box::new),
        },
        Err(Error::Telegram { source }) if !source.is_connection_failure() => {
            ActorResponse::Avatar {
                avatar,
                media: None,
            }
        }
        Err(Error::TelegramActorCancelled) => ActorResponse::Cancelled,
        Err(error) => ActorResponse::Failed(Box::new(error)),
    }
}

pub(super) async fn preview(
    state: &mut TelegramState,
    cancellation: &ActorCancellation,
    chat: intuigram_lib::ChatId,
    message: intuigram_lib::MessageId,
) -> ActorResponse {
    let result = until_cancelled(
        async {
            state
                .backend
                .client
                .download_media_preview(chat, message)
                .await
                .context(TelegramSnafu)
        },
        cancellation,
    )
    .await;
    match result {
        Ok(media) => ActorResponse::MediaPreview {
            chat,
            message,
            media: media.map(Box::new),
        },
        Err(Error::Telegram { source }) if !source.is_connection_failure() => {
            ActorResponse::MediaPreview {
                chat,
                message,
                media: None,
            }
        }
        Err(Error::TelegramActorCancelled) => ActorResponse::Cancelled,
        Err(error) => ActorResponse::Failed(Box::new(error)),
    }
}

pub(super) async fn download(
    state: &mut TelegramState,
    cancellation: &ActorCancellation,
    chat: intuigram_lib::ChatId,
    message: intuigram_lib::MessageId,
    destination: Option<String>,
) -> ActorResponse {
    let result = until_cancelled(
        async {
            state
                .backend
                .client
                .download_media(chat, message)
                .await
                .context(TelegramSnafu)
        },
        cancellation,
    )
    .await;
    match result {
        Ok(media) => ActorResponse::MediaDownload {
            chat,
            message,
            destination,
            media: Box::new(media),
        },
        Err(Error::Telegram { source }) if !source.is_connection_failure() => {
            ActorResponse::Complete(Box::new(AdapterBatch {
                event: Some(AdapterEvent::OperationFailed(source.to_string())),
                peers: intuigram_telegram::PeerDirectory::default(),
            }))
        }
        Err(Error::TelegramActorCancelled) => ActorResponse::Cancelled,
        Err(error) => ActorResponse::Failed(Box::new(error)),
    }
}

pub(super) async fn offline(
    state: &mut TelegramState,
    cancellation: &ActorCancellation,
    target: OfflineMediaTarget,
) -> ActorResponse {
    let result = until_cancelled(
        async {
            state
                .backend
                .client
                .download_media(target.chat, target.message)
                .await
                .context(TelegramSnafu)
        },
        cancellation,
    )
    .await;
    match result {
        Ok(media) => ActorResponse::MediaOffline {
            target,
            media: Some(Box::new(media)),
        },
        Err(Error::Telegram { source }) if !source.is_connection_failure() => {
            ActorResponse::Complete(Box::new(AdapterBatch {
                event: Some(AdapterEvent::MediaCacheOfflineFailed(OfflineMediaFailure {
                    chat: target.chat,
                    message: Some(target.message),
                    reason: source.to_string(),
                })),
                peers: intuigram_telegram::PeerDirectory::default(),
            }))
        }
        Err(Error::TelegramActorCancelled) => ActorResponse::Cancelled,
        Err(error) => ActorResponse::Failed(Box::new(error)),
    }
}
