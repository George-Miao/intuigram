use super::*;

mod refresh;
#[cfg(test)]
mod tests;
mod todo;

fn telegram_message_id(message: MessageId) -> Result<i32> {
    i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
        message_id: message.0,
    })
}

impl Client {
    async fn refreshed_family(
        &mut self,
        chat: ChatId,
        message: MessageId,
        family: &'static str,
        matches_family: impl FnOnce(&tl::enums::MessageMedia) -> bool,
    ) -> Result<MediaCard> {
        let media = self.message_media(chat, message).await?;
        if !matches_family(&media) {
            return SpecializedMediaUnavailableSnafu {
                message_id: message.0,
                family,
            }
            .fail();
        }
        Ok(normalize_media(&media))
    }
}
