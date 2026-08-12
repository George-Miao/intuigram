use intuigram_lib::{AdapterEvent, OfflineMediaFailure, OfflineMediaTarget};
use snafu::ResultExt;

use super::super::super::{AdapterBatch, Error, TelegramSnafu};
use super::EffectCancellation;
use super::actor::ActorResponse;
use super::cancellation::{ActorCancellation, until_effect_cancelled};

pub(super) async fn avatar(
    client: &mut intuigram_telegram::MediaClient,
    cancellation: &ActorCancellation,
    effect_cancellation: &EffectCancellation,
    avatar: intuigram_lib::AvatarRef,
) -> ActorResponse {
    let result = until_effect_cancelled(
        async { client.download_avatar(avatar).await.context(TelegramSnafu) },
        cancellation,
        effect_cancellation,
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
    client: &mut intuigram_telegram::MediaClient,
    cancellation: &ActorCancellation,
    effect_cancellation: &EffectCancellation,
    chat: intuigram_lib::ChatId,
    message: intuigram_lib::MessageId,
    locator: Option<intuigram_lib::MediaLocator>,
) -> ActorResponse {
    let result = until_effect_cancelled(
        async {
            client
                .download_media_preview(chat, message, locator.as_ref())
                .await
                .context(TelegramSnafu)
        },
        cancellation,
        effect_cancellation,
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
    client: &mut intuigram_telegram::MediaClient,
    cancellation: &ActorCancellation,
    effect_cancellation: &EffectCancellation,
    chat: intuigram_lib::ChatId,
    message: intuigram_lib::MessageId,
    destination: Option<String>,
    locator: Option<intuigram_lib::MediaLocator>,
) -> ActorResponse {
    let result = until_effect_cancelled(
        async {
            client
                .download_media(chat, message, locator.as_ref())
                .await
                .context(TelegramSnafu)
        },
        cancellation,
        effect_cancellation,
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
    client: &mut intuigram_telegram::MediaClient,
    cancellation: &ActorCancellation,
    effect_cancellation: &EffectCancellation,
    target: OfflineMediaTarget,
    locator: Option<intuigram_lib::MediaLocator>,
) -> ActorResponse {
    let result = until_effect_cancelled(
        async {
            client
                .download_media(target.chat, target.message, locator.as_ref())
                .await
                .context(TelegramSnafu)
        },
        cancellation,
        effect_cancellation,
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
