use super::*;

mod cdn;
mod location;
#[cfg(test)]
mod tests;
mod transfer;

use location::{download_location, download_locator, expired_file_reference};
use transfer::MAX_INLINE_PREVIEW_BYTES;

impl Client {
    /// Fetches the current small cloud avatar for a normalized peer.
    pub async fn download_avatar(&mut self, avatar: AvatarRef) -> Result<Option<DownloadedMedia>> {
        let Some((location, dc_id)) = self.peers.avatar_location(avatar)? else {
            return Ok(None);
        };
        let media = self.download_unknown(dc_id, location).await?;
        Ok(Some(media))
    }

    /// Fetches one Message's full photo or document bytes from Telegram.
    pub async fn download_media(
        &mut self,
        chat: ChatId,
        message: MessageId,
        locator: Option<&MediaLocator>,
    ) -> Result<DownloadedMedia> {
        let download = match locator {
            Some(locator) => download_locator(locator, None),
            None => {
                let media = self.message_media(chat, message).await?;
                download_location(media, message, None)?
            }
        }
        .context(DownloadMediaUnavailableSnafu {
            message_id: message.0,
        })?;
        match self.download_location(download).await {
            Err(error) if locator.is_some() && expired_file_reference(&error) => {
                let media = self.message_media(chat, message).await?;
                let download = download_location(media, message, None)?.context(
                    DownloadMediaUnavailableSnafu {
                        message_id: message.0,
                    },
                )?;
                self.download_location(download).await
            }
            result => result,
        }
    }

    /// Fetches bounded bytes suitable for an automatic inline preview.
    pub async fn download_media_preview(
        &mut self,
        chat: ChatId,
        message: MessageId,
        locator: Option<&MediaLocator>,
    ) -> Result<Option<DownloadedMedia>> {
        let download = match locator {
            Some(locator) => download_locator(locator, Some(MAX_INLINE_PREVIEW_BYTES)),
            None => {
                let media = self.message_media(chat, message).await?;
                download_location(media, message, Some(MAX_INLINE_PREVIEW_BYTES))?
            }
        };
        let Some(download) = download else {
            return Ok(None);
        };
        match self.download_location(download).await {
            Err(error) if locator.is_some() && expired_file_reference(&error) => {
                let media = self.message_media(chat, message).await?;
                let Some(download) =
                    download_location(media, message, Some(MAX_INLINE_PREVIEW_BYTES))?
                else {
                    return Ok(None);
                };
                self.download_location(download).await.map(Some)
            }
            result => result.map(Some),
        }
    }

    pub(super) async fn message_media(
        &mut self,
        chat: ChatId,
        message: MessageId,
    ) -> Result<tl::enums::MessageMedia> {
        let id = i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
            message_id: message.0,
        })?;
        let input = vec![tl::types::InputMessageId { id }.into()];
        let peer = self.peers.resolve(chat)?;
        let response = match peer {
            tl::enums::InputPeer::Channel(channel) => self
                .connection
                .invoke(&tl::functions::channels::GetMessages {
                    channel: tl::types::InputChannel {
                        channel_id: channel.channel_id,
                        access_hash: channel.access_hash,
                    }
                    .into(),
                    id: input,
                })
                .await
                .context(InvokeSnafu)?,
            _ => self
                .connection
                .invoke(&tl::functions::messages::GetMessages { id: input })
                .await
                .context(InvokeSnafu)?,
        };
        let (messages, chats, users) = message_parts(response);
        self.update_peer_cache(&chats, &users);
        messages
            .into_iter()
            .find(|candidate| candidate.id() == id)
            .and_then(|candidate| match candidate {
                tl::enums::Message::Message(message) => message.media,
                tl::enums::Message::Empty(_) | tl::enums::Message::Service(_) => None,
            })
            .context(DownloadMessageUnavailableSnafu {
                message_id: message.0,
            })
    }
}
