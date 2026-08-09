use std::cell::RefCell;

use intuigram_app::{AdapterEvent, Effect};
use intuigram_store::AccountStore;

use super::super::{AttachmentStore, BackendOutput, DownloadStore, Result};
use crate::application::runtime::{EffectRoute, effect_route};

mod media;
mod picker;
mod platform;
mod storage;
mod upload;

pub(super) use upload::{attachment_payloads, prepare_rich_media};

pub(super) struct State {
    pub(super) attachments: AttachmentStore,
    pub(super) downloaded: DownloadStore,
    pub(super) downloads: intuigram_media::DownloadDirectory,
    pub(super) media_cache: intuigram_media::MediaCache,
    pub(super) path_picker: Option<intuigram_config::ExternalCommand>,
}

impl State {
    pub(super) fn new(
        downloads: intuigram_media::DownloadDirectory,
        media_cache: intuigram_media::MediaCache,
        path_picker: Option<intuigram_config::ExternalCommand>,
    ) -> Self {
        Self {
            attachments: AttachmentStore::default(),
            downloaded: DownloadStore::default(),
            downloads,
            media_cache,
            path_picker,
        }
    }
}

pub(super) const fn handles(effect: &Effect) -> bool {
    matches!(effect, Effect::SetChatMediaOffline(_)) || effect_route(effect).is_local()
}

pub(super) async fn execute(
    effect: Effect,
    store: &AccountStore,
    state: &RefCell<State>,
) -> Result<BackendOutput> {
    let event = if matches!(effect, Effect::SetChatMediaOffline(_)) {
        storage::execute(effect, store, state).await?
    } else {
        match effect_route(&effect) {
            EffectRoute::LocalIndependent => platform::execute(effect, state).await?,
            EffectRoute::LocalOrdered => storage::execute(effect, store, state).await?,
            EffectRoute::Telegram => {
                unreachable!("Telegram effects do not reach the local executor")
            }
        }
    };
    Ok(BackendOutput::event(event))
}

pub(super) fn observe_completion(
    effect: &Effect,
    event: &Option<AdapterEvent>,
    state: &RefCell<State>,
) {
    let mut state = state.borrow_mut();
    let succeeded = !matches!(event, Some(AdapterEvent::MessageEditFailed { .. }));
    if succeeded
        && let Effect::SendMessage { attachments, .. } | Effect::EditMessage { attachments, .. } =
            effect
    {
        for id in attachments {
            state.attachments.payloads.remove(id);
        }
    }
    if let Some(AdapterEvent::DownloadReady { download, .. }) = event {
        state.downloaded.next_id = state.downloaded.next_id.max(download.id.0);
        state
            .downloaded
            .paths
            .insert(download.id, download.path.clone().into());
    }
}

pub(super) async fn cached_preview(
    state: &RefCell<State>,
    chat: intuigram_app::ChatId,
    message: intuigram_app::MessageId,
) -> Result<Option<intuigram_app::InlineImage>> {
    media::cached_preview(state, chat, message).await
}

pub(super) async fn cached_avatar(
    state: &RefCell<State>,
    avatar: intuigram_app::AvatarRef,
) -> Result<Option<intuigram_app::InlineImage>> {
    media::cached_avatar(state, avatar).await
}

pub(super) async fn cached_original(
    state: &RefCell<State>,
    target: intuigram_app::OfflineMediaTarget,
) -> Result<Option<intuigram_telegram::DownloadedMedia>> {
    media::cached_original(state, target).await
}

pub(super) async fn finish_preview(
    state: &RefCell<State>,
    chat: intuigram_app::ChatId,
    message: intuigram_app::MessageId,
    media: Option<intuigram_telegram::DownloadedMedia>,
) -> Result<AdapterEvent> {
    media::finish_preview(state, chat, message, media).await
}

pub(super) async fn finish_avatar(
    state: &RefCell<State>,
    avatar: intuigram_app::AvatarRef,
    media: Option<intuigram_telegram::DownloadedMedia>,
) -> Result<AdapterEvent> {
    media::finish_avatar(state, avatar, media).await
}

pub(super) async fn finish_offline_media(
    state: &RefCell<State>,
    target: intuigram_app::OfflineMediaTarget,
    media: Option<intuigram_telegram::DownloadedMedia>,
) -> Result<AdapterEvent> {
    media::finish_offline_media(state, target, media).await
}

pub(super) async fn finish_download(
    state: &RefCell<State>,
    chat: intuigram_app::ChatId,
    message: intuigram_app::MessageId,
    destination: Option<String>,
    media: intuigram_telegram::DownloadedMedia,
) -> Result<AdapterEvent> {
    media::finish_download(state, chat, message, destination, media).await
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use intuigram_app::{ChatId, MessageId};

    use super::*;

    #[test]
    fn telegram_history_is_not_classified_as_main_thread_platform_work() {
        assert!(!handles(&Effect::LoadChat {
            chat: ChatId(7),
            selection: None,
            transcript_anchors: Vec::new(),
        }));
    }

    #[test]
    fn storage_and_native_io_stay_outside_the_telegram_actor() {
        assert!(handles(&Effect::SaveDraft {
            chat: ChatId(7),
            thread_root: None,
            saved_peer: None,
            text: "draft".to_owned(),
            reply_to: Some(MessageId(9)),
        }));
        assert!(handles(&Effect::ReadClipboard {
            chat: ChatId(7),
            thread_root: None,
            saved_peer: None,
        }));
        assert!(handles(&Effect::SetChatMediaOffline(
            intuigram_app::OfflineMediaPolicy {
                chat: ChatId(7),
                keep: true,
            },
        )));
    }

    #[test]
    fn rich_media_file_is_read_before_actor_delivery() {
        let mut file = tempfile::NamedTempFile::new().expect("temporary media should be created");
        file.write_all(b"prepared media")
            .expect("temporary media should be written");
        let effect = Effect::SendRichMediaFile {
            chat: ChatId(7),
            path: file.path().display().to_string(),
            kind: intuigram_app::RichMediaUploadKind::File,
            local_id: MessageId(-1),
            reply_to: None,
            thread_root: None,
            saved_peer: None,
        };
        let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

        let prepared = runtime
            .block_on(prepare_rich_media(&effect))
            .expect("media preparation should succeed")
            .expect("file media should require preparation");

        assert_eq!(prepared.bytes, b"prepared media");
        assert_eq!(prepared.kind, intuigram_app::RichMediaUploadKind::File);
    }
}
