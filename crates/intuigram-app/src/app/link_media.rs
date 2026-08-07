use super::*;

impl App {
    pub(super) fn apply_link_media_event(&mut self, event: AdapterEvent) -> Option<Effect> {
        match event {
            AdapterEvent::TelegramLinkResolved { chat } => {
                let chat_id = chat.id;
                if let Some(existing) = self
                    .all_chats
                    .iter_mut()
                    .find(|candidate| candidate.id == chat_id)
                {
                    *existing = chat;
                } else {
                    self.all_chats.push(chat);
                }
                self.refresh_folder_chats(Some(chat_id));
                self.focus_composer_at_anchor();
                self.request_chat_load(chat_id)
            }
            AdapterEvent::DownloadReady { chat, download } => {
                let launch_warning = download.reveal_only.then_some(
                    " Launchable content will only be revealed in its folder.".to_owned(),
                );
                self.view.notice = Some(format!(
                    "Downloaded to {}{}",
                    download.path,
                    launch_warning.as_deref().unwrap_or_default()
                ));
                self.view.downloads.retain(|existing| {
                    existing.chat != chat || existing.message != download.message
                });
                self.view.downloads.push(download);
                None
            }
            AdapterEvent::MediaPreviewReady(preview) => {
                let key = PreviewKey {
                    chat: preview.chat,
                    message: preview.message,
                };
                self.store_media_preview(preview);
                self.complete_media_preview(key)
            }
            AdapterEvent::MediaPreviewFailed { chat, message } => {
                self.complete_media_preview(PreviewKey { chat, message })
            }
            _ => None,
        }
    }

    pub(super) fn open_active_link(&mut self) -> Option<Effect> {
        let link = self
            .view
            .active_message
            .and_then(|index| self.view.messages.get(index))
            .and_then(active_link)?;
        if link.suspicious {
            self.view.link_confirmation = Some(link);
            return None;
        }
        Self::link_effect(link)
    }

    pub(super) fn confirm_active_link(&mut self) -> Option<Effect> {
        self.view
            .link_confirmation
            .take()
            .and_then(Self::link_effect)
    }

    pub(super) fn download_active_media(&self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        let message = self
            .view
            .active_message
            .and_then(|index| self.view.messages.get(index))?;
        message.details.media.as_ref()?.remote_id.as_ref()?;
        Some(Effect::DownloadMedia {
            chat,
            message: message.id,
            destination: None,
        })
    }

    pub(super) fn open_save_as(&mut self) {
        let Some(name) = self
            .view
            .active_message
            .and_then(|index| self.view.messages.get(index))
            .and_then(|message| message.details.media.as_ref())
            .and_then(|media| media.remote_id.as_ref().map(|_| media.title.clone()))
        else {
            return;
        };
        self.view.save_as = Some(SaveAsView { destination: name });
    }

    pub(super) fn apply_save_as_action(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::ConfirmSaveAs => self.confirm_save_as(),
            Action::Cancel | Action::SaveAs => {
                self.view.save_as = None;
                None
            }
            _ => None,
        }
    }

    pub(super) fn confirm_save_as(&mut self) -> Option<Effect> {
        let destination = self.view.save_as.take()?.destination;
        if destination.is_empty() {
            return None;
        }
        let chat = self.active_chat_id()?;
        let message = self.active_message_id()?;
        Some(Effect::DownloadMedia {
            chat,
            message,
            destination: Some(destination),
        })
    }

    pub(super) fn open_download(&self) -> Option<Effect> {
        let active = self.active_message_id()?;
        let chat = self.active_chat_id()?;
        let download = self
            .view
            .downloads
            .iter()
            .find(|download| download.chat == chat && download.message == active)?;
        Some(Effect::OpenDownload {
            download: download.id,
            reveal: download.reveal_only,
        })
    }

    fn link_effect(link: LinkTarget) -> Option<Effect> {
        link.telegram_username.map_or_else(
            || Some(Effect::OpenExternalLink { url: link.url }),
            |username| Some(Effect::ResolveTelegramUsername { username }),
        )
    }
}
