use super::*;

/// Telegram-owned media library queried for composer selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaLibraryKind {
    /// Recently used stickers.
    Stickers,
    /// Saved animated GIF documents.
    Gifs,
    /// Custom emoji matching an emoji or keyword query.
    CustomEmoji,
}

/// One sendable entry from a Telegram-owned media library.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaLibraryEntry {
    /// Stable Telegram document identifier.
    pub id: i64,
    /// Human-readable emoji, filename, or MIME fallback.
    pub label: String,
    kind: MediaLibraryKind,
    access_hash: i64,
    file_reference: Vec<u8>,
}

/// One Telegram contact card submission.
pub struct ContactCardSend {
    /// Destination Chat.
    pub chat: ChatId,
    /// Telegram-compatible telephone number.
    pub phone_number: String,
    /// Contact first name.
    pub first_name: String,
    /// Optional contact last name.
    pub last_name: String,
    /// Direct reply target.
    pub reply_to: Option<MessageId>,
    /// Active Thread root.
    pub thread_root: Option<MessageId>,
    /// Stable Message idempotency identifier.
    pub random_id: i64,
}

impl Client {
    /// Loads recent stickers, saved GIFs, or searched custom emoji.
    pub async fn browse_media(
        &mut self,
        kind: MediaLibraryKind,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MediaLibraryEntry>> {
        let documents = match kind {
            MediaLibraryKind::Stickers if query.is_empty() => {
                match self
                    .connection
                    .invoke(&tl::functions::messages::GetRecentStickers {
                        attached: false,
                        hash: 0,
                    })
                    .await
                    .context(InvokeSnafu)?
                {
                    tl::enums::messages::RecentStickers::Stickers(stickers) => stickers.stickers,
                    tl::enums::messages::RecentStickers::NotModified => Vec::new(),
                }
            }
            MediaLibraryKind::Stickers => {
                match self
                    .connection
                    .invoke(&tl::functions::messages::SearchStickers {
                        emojis: false,
                        q: query.to_owned(),
                        emoticon: String::new(),
                        lang_code: Vec::new(),
                        offset: 0,
                        limit: i32::try_from(limit).unwrap_or(i32::MAX),
                        hash: 0,
                    })
                    .await
                    .context(InvokeSnafu)?
                {
                    tl::enums::messages::FoundStickers::Stickers(found) => found.stickers,
                    tl::enums::messages::FoundStickers::NotModified(_) => Vec::new(),
                }
            }
            MediaLibraryKind::Gifs => {
                match self
                    .connection
                    .invoke(&tl::functions::messages::GetSavedGifs { hash: 0 })
                    .await
                    .context(InvokeSnafu)?
                {
                    tl::enums::messages::SavedGifs::Gifs(gifs) => gifs.gifs,
                    tl::enums::messages::SavedGifs::NotModified => Vec::new(),
                }
            }
            MediaLibraryKind::CustomEmoji => {
                let ids = match self
                    .connection
                    .invoke(&tl::functions::messages::SearchCustomEmoji {
                        emoticon: query.to_owned(),
                        hash: 0,
                    })
                    .await
                    .context(InvokeSnafu)?
                {
                    tl::enums::EmojiList::List(list) => list.document_id,
                    tl::enums::EmojiList::NotModified => Vec::new(),
                };
                self.connection
                    .invoke(&tl::functions::messages::GetCustomEmojiDocuments {
                        document_id: ids.into_iter().take(limit).collect(),
                    })
                    .await
                    .context(InvokeSnafu)?
            }
        };
        Ok(documents
            .into_iter()
            .filter_map(|document| library_entry(document, kind))
            .take(limit)
            .collect())
    }

    /// Sends one previously browsed library entry to a Chat.
    pub async fn send_library_media(
        &mut self,
        chat: ChatId,
        entry: &MediaLibraryEntry,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
        random_id: i64,
    ) -> Result<()> {
        if entry.kind == MediaLibraryKind::CustomEmoji {
            let text = if entry.label.is_empty() {
                "🙂".to_owned()
            } else {
                entry.label.clone()
            };
            return self
                .send_text(TextSend {
                    chat,
                    entities: vec![TextEntity {
                        offset: 0,
                        length: text.encode_utf16().count(),
                        kind: TextEntityKind::CustomEmoji {
                            document_id: entry.id,
                        },
                    }],
                    text,
                    link_preview: false,
                    reply_to,
                    thread_root,
                    random_id,
                    schedule_date: None,
                })
                .await;
        }
        let peer = self.peers.resolve(chat)?;
        let media = tl::types::InputMediaDocument {
            spoiler: false,
            id: tl::types::InputDocument {
                id: entry.id,
                access_hash: entry.access_hash,
                file_reference: entry.file_reference.clone(),
            }
            .into(),
            video_cover: None,
            video_timestamp: None,
            ttl_seconds: None,
            query: None,
        }
        .into();
        self.send_input_media(peer, media, String::new(), reply_to, thread_root, random_id)
            .await
    }

    /// Sends a Telegram contact card.
    pub async fn send_contact(&mut self, request: ContactCardSend) -> Result<()> {
        let ContactCardSend {
            chat,
            phone_number,
            first_name,
            last_name,
            reply_to,
            thread_root,
            random_id,
        } = request;
        let peer = self.peers.resolve(chat)?;
        let media = tl::types::InputMediaContact {
            phone_number,
            first_name,
            last_name,
            vcard: String::new(),
        }
        .into();
        self.send_input_media(peer, media, String::new(), reply_to, thread_root, random_id)
            .await
    }

    async fn send_input_media(
        &mut self,
        peer: tl::enums::InputPeer,
        media: tl::enums::InputMedia,
        message: String,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
        random_id: i64,
    ) -> Result<()> {
        self.connection
            .invoke(&tl::functions::messages::SendMedia {
                silent: false,
                background: false,
                clear_draft: true,
                noforwards: false,
                update_stickersets_order: false,
                invert_media: false,
                allow_paid_floodskip: false,
                peer,
                reply_to: input_reply_to(reply_to, thread_root)?,
                media,
                message,
                random_id,
                reply_markup: None,
                entities: None,
                schedule_date: None,
                schedule_repeat_period: None,
                send_as: None,
                quick_reply_shortcut: None,
                effect: None,
                allow_paid_stars: None,
                suggested_post: None,
            })
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }
}

fn library_entry(
    document: tl::enums::Document,
    kind: MediaLibraryKind,
) -> Option<MediaLibraryEntry> {
    let tl::enums::Document::Document(document) = document else {
        return None;
    };
    let label = document
        .attributes
        .iter()
        .find_map(|attribute| match attribute {
            tl::enums::DocumentAttribute::Sticker(sticker) => Some(sticker.alt.clone()),
            tl::enums::DocumentAttribute::CustomEmoji(emoji) => Some(emoji.alt.clone()),
            tl::enums::DocumentAttribute::Filename(file) => Some(file.file_name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| document.mime_type.clone());
    Some(MediaLibraryEntry {
        id: document.id,
        label,
        kind,
        access_hash: document.access_hash,
        file_reference: document.file_reference,
    })
}
