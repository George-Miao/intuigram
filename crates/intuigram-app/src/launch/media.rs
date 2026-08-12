use super::{Command, Result};
use crate::{Maintenance, parse_media_maintenance};

impl Command {
    /// Browses stickers, GIFs, or custom emoji.
    pub fn media_browse(kind: String, query: String) -> Result<Self> {
        Self::media("media browse", "browse", [kind, query])
    }

    /// Sends one item from a media-library query.
    pub fn media_send(chat: String, kind: String, index: String, query: String) -> Result<Self> {
        Self::media("media send", "send", [chat, kind, index, query])
    }

    /// Sends voice, video-note, Sticker, GIF, or custom-emoji media.
    pub fn media_file(chat: String, kind: String, path: String) -> Result<Self> {
        Self::media("media file", "file", [chat, kind, path])
    }

    /// Records voice or a video note with ffmpeg, then sends it.
    pub fn media_record(
        chat: String,
        kind: String,
        seconds: String,
        device: String,
    ) -> Result<Self> {
        Self::media("media record", "record", [chat, kind, seconds, device])
    }

    /// Shares a Telegram contact card.
    pub fn media_contact(
        chat: String,
        phone: String,
        first_name: String,
        last_name: String,
    ) -> Result<Self> {
        Self::media(
            "media contact",
            "contact",
            [chat, phone, first_name, last_name],
        )
    }

    fn media<const N: usize>(label: &str, action: &str, values: [String; N]) -> Result<Self> {
        let command = parse_media_maintenance(&mut values.into_iter(), action, label)?;
        Ok(Self::maintenance(Maintenance::RichMedia(command)))
    }
}
