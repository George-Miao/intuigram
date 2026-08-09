use super::*;

impl App {
    pub(super) fn open_rich_media(&mut self) {
        if self.view.focus == Focus::Composer
            && self.view.active_chat.is_some()
            && self.view.connection == ConnectionState::Connected
        {
            self.view.rich_media = Some(RichMediaComposerView {
                mode: RichMediaComposerMode::Menu,
                selected: 0,
                pending: false,
            });
        }
    }

    pub(super) fn apply_rich_media_action(&mut self, action: Action) -> Option<Effect> {
        if action != Action::Cancel
            && self
                .view
                .rich_media
                .as_ref()
                .is_some_and(|composer| composer.pending)
        {
            return None;
        }
        match action {
            Action::MoveUp | Action::MoveDown => {
                let count = self.rich_media_row_count();
                if let Some(composer) = &mut self.view.rich_media {
                    composer.selected =
                        move_index(Some(composer.selected), count, action == Action::MoveDown)
                            .unwrap_or(0);
                }
                None
            }
            Action::CycleRichMediaKind => {
                if let Some(RichMediaComposerView {
                    mode: RichMediaComposerMode::File { kind, .. },
                    selected: 1,
                    ..
                }) = &mut self.view.rich_media
                {
                    *kind = kind.next();
                }
                None
            }
            Action::ChooseRichMedia => self.choose_rich_media(),
            Action::Cancel | Action::OpenRichMedia => {
                let nested = self
                    .view
                    .rich_media
                    .as_ref()
                    .is_some_and(|composer| composer.mode != RichMediaComposerMode::Menu);
                if nested {
                    if let Some(composer) = &mut self.view.rich_media {
                        composer.mode = RichMediaComposerMode::Menu;
                        composer.selected = 0;
                    }
                } else {
                    self.view.rich_media = None;
                }
                None
            }
            _ => None,
        }
    }

    pub(super) fn insert_rich_media_text(&mut self, text: &str) -> bool {
        let Some(composer) = &mut self.view.rich_media else {
            return false;
        };
        if composer.pending {
            return true;
        }
        if let RichMediaComposerMode::PlaceSearch { results, .. } = &mut composer.mode
            && composer.selected < 2
        {
            results.clear();
        }
        let Some(field) = rich_media_field(composer) else {
            return true;
        };
        field.push_str(text);
        true
    }

    pub(super) fn backspace_rich_media_text(&mut self) -> bool {
        let Some(composer) = &mut self.view.rich_media else {
            return false;
        };
        if composer.pending {
            return true;
        }
        if let RichMediaComposerMode::PlaceSearch { results, .. } = &mut composer.mode
            && composer.selected < 2
        {
            results.clear();
        }
        let Some(field) = rich_media_field(composer) else {
            return true;
        };
        field.pop();
        true
    }

    pub(super) fn apply_rich_media_event(&mut self, event: AdapterEvent) {
        match event {
            AdapterEvent::RichMediaLibraryReady { kind, items } => {
                if let Some(composer) = &mut self.view.rich_media
                    && matches!(composer.mode, RichMediaComposerMode::Library { kind: current, .. } if current == kind)
                {
                    composer.mode = RichMediaComposerMode::Library { kind, items };
                    composer.selected = 0;
                    composer.pending = false;
                }
                self.view.notice = None;
            }
            AdapterEvent::RichMediaLibraryFailed(reason) => {
                if let Some(composer) = &mut self.view.rich_media {
                    composer.pending = false;
                }
                self.view.notice = Some(reason);
            }
            event @ (AdapterEvent::PlaceSearchReady { .. }
            | AdapterEvent::PlaceSearchFailed { .. }) => self.apply_place_search_event(event),
            AdapterEvent::RichMediaAcknowledged {
                chat,
                local_id,
                server_id,
            } => {
                self.acknowledge_message(chat, local_id, server_id);
                self.view.notice = None;
            }
            AdapterEvent::RichMediaFailed {
                chat,
                local_id,
                reason,
            } => {
                self.update_delivery(chat, local_id, DeliveryState::Failed);
                self.view.notice = Some(reason);
            }
            _ => {}
        }
    }

    fn choose_rich_media(&mut self) -> Option<Effect> {
        let composer = self.view.rich_media.as_ref()?.clone();
        match composer.mode {
            RichMediaComposerMode::Menu => self.choose_rich_media_menu(composer.selected),
            RichMediaComposerMode::Library { kind, items } => {
                let item = items.get(composer.selected)?.clone();
                let label = if item.label.is_empty() {
                    format!("[{kind:?}]")
                } else {
                    item.label.clone()
                };
                self.queue_rich_media(
                    label,
                    |chat, local_id, reply_to, thread_root, saved_peer| Effect::SendLibraryMedia {
                        chat,
                        item: item.id,
                        local_id,
                        reply_to,
                        thread_root,
                        saved_peer,
                    },
                )
            }
            RichMediaComposerMode::File { path, kind } if !path.trim().is_empty() => {
                let display = std::path::Path::new(path.trim()).file_name().map_or_else(
                    || path.trim().to_owned(),
                    |name| name.to_string_lossy().into(),
                );
                self.queue_rich_media(
                    format!("[{kind:?}] {display}"),
                    |chat, local_id, reply_to, thread_root, saved_peer| Effect::SendRichMediaFile {
                        chat,
                        path: path.trim().to_owned(),
                        kind,
                        local_id,
                        reply_to,
                        thread_root,
                        saved_peer,
                    },
                )
            }
            RichMediaComposerMode::Recording {
                kind,
                seconds,
                device,
            } if matches!(
                kind,
                RichMediaUploadKind::Voice | RichMediaUploadKind::VideoNote
            ) && !device.trim().is_empty() =>
            {
                let seconds = seconds.parse::<u32>().ok().filter(|value| *value > 0)?;
                self.queue_rich_media(
                    format!("[{kind:?}]"),
                    |chat, local_id, reply_to, thread_root, saved_peer| Effect::RecordRichMedia {
                        chat,
                        kind,
                        seconds,
                        device: device.trim().to_owned(),
                        local_id,
                        reply_to,
                        thread_root,
                        saved_peer,
                    },
                )
            }
            RichMediaComposerMode::Contact {
                phone,
                first_name,
                last_name,
            } if !phone.trim().is_empty() && !first_name.trim().is_empty() => self
                .queue_rich_media(
                    format!("[Contact] {} {}", first_name.trim(), last_name.trim()),
                    |chat, local_id, reply_to, thread_root, saved_peer| Effect::SendContact {
                        chat,
                        phone: phone.trim().to_owned(),
                        first_name: first_name.trim().to_owned(),
                        last_name: last_name.trim().to_owned(),
                        local_id,
                        reply_to,
                        thread_root,
                        saved_peer,
                    },
                ),
            RichMediaComposerMode::StaticLocation { .. }
            | RichMediaComposerMode::PlaceSearch { .. } => self.choose_location(composer),
            _ => None,
        }
    }

    fn choose_rich_media_menu(&mut self, selected: usize) -> Option<Effect> {
        let composer = self.view.rich_media.as_mut()?;
        let library = match selected {
            0 => Some(RichMediaLibraryKind::Stickers),
            1 => Some(RichMediaLibraryKind::Gifs),
            2 => Some(RichMediaLibraryKind::CustomEmoji),
            _ => None,
        };
        if let Some(kind) = library {
            composer.mode = RichMediaComposerMode::Library {
                kind,
                items: Vec::new(),
            };
            composer.selected = 0;
            composer.pending = true;
            return Some(Effect::BrowseRichMedia { kind });
        }
        composer.selected = 0;
        composer.mode = match selected {
            3 => RichMediaComposerMode::File {
                path: String::new(),
                kind: RichMediaUploadKind::File,
            },
            4 => RichMediaComposerMode::Recording {
                kind: RichMediaUploadKind::Voice,
                seconds: String::new(),
                device: String::new(),
            },
            5 => RichMediaComposerMode::Recording {
                kind: RichMediaUploadKind::VideoNote,
                seconds: String::new(),
                device: String::new(),
            },
            6 => RichMediaComposerMode::Contact {
                phone: String::new(),
                first_name: String::new(),
                last_name: String::new(),
            },
            7 => RichMediaComposerMode::StaticLocation {
                input: String::new(),
            },
            8 => RichMediaComposerMode::PlaceSearch {
                query: String::new(),
                near: String::new(),
                results: Vec::new(),
            },
            _ => return None,
        };
        None
    }

    pub(super) fn queue_rich_media(
        &mut self,
        body: String,
        effect: impl FnOnce(
            ChatId,
            MessageId,
            Option<MessageId>,
            Option<MessageId>,
            Option<ChatId>,
        ) -> Effect,
    ) -> Option<Effect> {
        self.queue_rich_media_with_card(body, None, effect)
    }

    pub(super) fn queue_rich_media_with_card(
        &mut self,
        body: String,
        media: Option<MediaCard>,
        effect: impl FnOnce(
            ChatId,
            MessageId,
            Option<MessageId>,
            Option<MessageId>,
            Option<ChatId>,
        ) -> Effect,
    ) -> Option<Effect> {
        let key = self.active_history_key()?;
        self.next_local_message_id = self.next_local_message_id.saturating_sub(1);
        let local_id = MessageId(self.next_local_message_id);
        let reply_to = self.view.composer.reply_to;
        self.histories.entry(key).or_default().push(MessageView {
            id: local_id,
            sender: "You".to_owned(),
            body,
            timestamp: "now".to_owned(),
            direction: MessageDirection::Outgoing,
            delivery: DeliveryState::Pending,
            reply_to,
            details: MessageDetails {
                media,
                thread_root: key.thread,
                saved_peer: key.saved_peer,
                ..MessageDetails::default()
            },
        });
        self.refresh_active_history();
        self.view.rich_media = None;
        Some(effect(
            key.chat,
            local_id,
            reply_to,
            key.thread,
            key.saved_peer,
        ))
    }

    fn rich_media_row_count(&self) -> usize {
        match self.view.rich_media.as_ref().map(|composer| &composer.mode) {
            Some(RichMediaComposerMode::Menu) => 9,
            Some(RichMediaComposerMode::Library { items, .. }) => items.len(),
            Some(RichMediaComposerMode::File { .. })
            | Some(RichMediaComposerMode::Recording { .. }) => 2,
            Some(RichMediaComposerMode::Contact { .. }) => 3,
            Some(RichMediaComposerMode::StaticLocation { .. }) => 1,
            Some(RichMediaComposerMode::PlaceSearch { results, .. }) => 2 + results.len(),
            None => 0,
        }
    }
}

fn rich_media_field(composer: &mut RichMediaComposerView) -> Option<&mut String> {
    match (&mut composer.mode, composer.selected) {
        (RichMediaComposerMode::File { path, .. }, 0) => Some(path),
        (RichMediaComposerMode::Recording { seconds, .. }, 0) => Some(seconds),
        (RichMediaComposerMode::Recording { device, .. }, 1) => Some(device),
        (RichMediaComposerMode::Contact { phone, .. }, 0) => Some(phone),
        (RichMediaComposerMode::Contact { first_name, .. }, 1) => Some(first_name),
        (RichMediaComposerMode::Contact { last_name, .. }, 2) => Some(last_name),
        (RichMediaComposerMode::StaticLocation { input }, 0) => Some(input),
        (RichMediaComposerMode::PlaceSearch { query, .. }, 0) => Some(query),
        (RichMediaComposerMode::PlaceSearch { near, .. }, 1) => Some(near),
        _ => None,
    }
}
