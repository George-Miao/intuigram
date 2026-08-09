use super::*;

impl App {
    pub(super) fn choose_location(&mut self, composer: RichMediaComposerView) -> Option<Effect> {
        match composer.mode {
            RichMediaComposerMode::StaticLocation { input } => {
                let point = match parse_geo_point(&input) {
                    Ok(point) => point,
                    Err(error) => {
                        self.view.notice = Some(error.to_string());
                        return None;
                    }
                };
                let coordinates = point.coordinates();
                self.queue_rich_media_with_card(
                    format!("[Location] {coordinates}"),
                    Some(MediaCard {
                        kind: MediaKind::Location,
                        title: "Location".to_owned(),
                        description: coordinates,
                        details: Vec::new(),
                        poll: None,
                        specialized: None,
                        remote_id: None,
                    }),
                    |chat, local_id, reply_to, thread_root, saved_peer| {
                        Effect::SendStaticLocation {
                            chat,
                            point,
                            local_id,
                            reply_to,
                            thread_root,
                            saved_peer,
                        }
                    },
                )
            }
            RichMediaComposerMode::PlaceSearch {
                query: _,
                near: _,
                results,
            } if composer.selected >= 2 => {
                let venue = results.get(composer.selected - 2)?.clone();
                let body = format!("[{}] {}", venue.title, venue.address);
                self.queue_rich_media_with_card(
                    body,
                    Some(MediaCard {
                        kind: MediaKind::Venue,
                        title: venue.title.clone(),
                        description: venue.address.clone(),
                        details: vec![venue.point.coordinates()],
                        poll: None,
                        specialized: None,
                        remote_id: None,
                    }),
                    |chat, local_id, reply_to, thread_root, saved_peer| Effect::SendVenue {
                        chat,
                        venue,
                        local_id,
                        reply_to,
                        thread_root,
                        saved_peer,
                    },
                )
            }
            RichMediaComposerMode::PlaceSearch { query, near, .. } if !query.trim().is_empty() => {
                let near = if near.trim().is_empty() {
                    None
                } else {
                    match parse_geo_point(&near) {
                        Ok(point) => Some(point),
                        Err(error) => {
                            self.view.notice = Some(error.to_string());
                            return None;
                        }
                    }
                };
                let chat = self.active_chat_id()?;
                let query = query.trim().to_owned();
                if let Some(active) = &mut self.view.rich_media {
                    active.pending = true;
                }
                Some(Effect::SearchPlaces { chat, query, near })
            }
            _ => None,
        }
    }

    pub(super) fn apply_place_search_event(&mut self, event: AdapterEvent) {
        let (chat, query, near, result) = match event {
            AdapterEvent::PlaceSearchReady {
                chat,
                query,
                near,
                places,
            } => (chat, query, near, Ok(places)),
            AdapterEvent::PlaceSearchFailed {
                chat,
                query,
                near,
                reason,
            } => (chat, query, near, Err(reason)),
            _ => return,
        };
        let active_chat = self.active_chat_id();
        let Some(composer) = &mut self.view.rich_media else {
            return;
        };
        let RichMediaComposerMode::PlaceSearch {
            query: current,
            near: current_near,
            results,
        } = &mut composer.mode
        else {
            return;
        };
        let parsed_near = if current_near.trim().is_empty() {
            None
        } else {
            parse_geo_point(current_near).ok()
        };
        if active_chat != Some(chat) || *current != query || parsed_near != near {
            return;
        }
        composer.pending = false;
        match result {
            Ok(places) => {
                *results = places;
                composer.selected = if results.is_empty() { 0 } else { 2 };
                self.view.notice = results.is_empty().then(|| "No places found".to_owned());
            }
            Err(reason) => self.view.notice = Some(reason),
        }
    }
}
