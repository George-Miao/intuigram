impl Client {
    pub(super) async fn initialize(&mut self) -> Result<()> {
        let config = self
            .connection
            .invoke(&tl::functions::InvokeWithLayer {
                layer: tl::LAYER,
                query: tl::functions::InitConnection {
                    api_id: self.credentials.api_id,
                    device_model: "Terminal".to_owned(),
                    system_version: std::env::consts::OS.to_owned(),
                    app_version: env!("CARGO_PKG_VERSION").to_owned(),
                    system_lang_code: "en".to_owned(),
                    lang_pack: String::new(),
                    lang_code: "en".to_owned(),
                    proxy: None,
                    params: None,
                    query: tl::functions::help::GetConfig {},
                },
            })
            .await
            .context(InvokeSnafu)?;
        let tl::enums::Config::Config(config) = config;
        ensure_production_environment(config.test_mode)?;
        self.media_data_centers = media_data_centers(config.dc_options.clone());
        self.cdn_data_centers = cdn_data_centers(config.dc_options.clone());
        self.data_centers = direct_data_centers(config.dc_options);
        self.venue_search_username = config.venue_search_username;
        Ok(())
    }

    /// Reads Telegram's current per-data-center active media-operation limits.
    pub async fn media_limits(&mut self) -> Result<MediaLimits> {
        self.connection
            .invoke(&tl::functions::account::GetAutoDownloadSettings {})
            .await
            .map(normalize_media_limits)
            .context(InvokeSnafu)
    }

    pub(super) fn update_peer_cache(
        &mut self,
        chats: &[tl::enums::Chat],
        users: &[tl::enums::User],
    ) {
        self.peers.update(chats, users);
        for user in users {
            match user {
                tl::enums::User::User(user) => {
                    let id = ChatId(user.id);
                    self.names.insert(id, user_display_name(user));
                }
                tl::enums::User::Empty(user) => {
                    self.names
                        .insert(ChatId(user.id), "Inaccessible user".to_owned());
                }
            }
        }
        for chat in chats {
            match chat {
                tl::enums::Chat::Chat(chat) => {
                    let id = ChatId(-chat.id);
                    self.names.insert(id, chat.title.clone());
                }
                tl::enums::Chat::Channel(channel) => {
                    let id = ChatId(mark_channel_id(channel.id));
                    self.names.insert(id, channel.title.clone());
                }
                tl::enums::Chat::Forbidden(chat) => {
                    self.names.insert(ChatId(-chat.id), chat.title.clone());
                }
                tl::enums::Chat::ChannelForbidden(channel) => {
                    self.names
                        .insert(ChatId(mark_channel_id(channel.id)), channel.title.clone());
                }
                tl::enums::Chat::Empty(chat) => {
                    self.names
                        .insert(ChatId(-chat.id), "Inaccessible group".to_owned());
                }
            }
        }
    }
}
use super::*;
