use intuigram_lib::{AdapterEvent, Effect};

use super::TestSystem;
use crate::error::Result;

impl TestSystem {
    pub(super) fn handle_location_effect(&mut self, effect: Effect) -> Result<()> {
        match effect {
            Effect::SearchPlaces { chat, query, near } => {
                let places = self
                    .telegram
                    .search_places(chat, query.clone(), near)
                    .map_err(|error| self.scenario_error(error))?;
                self.application
                    .handle_adapter(AdapterEvent::PlaceSearchReady {
                        chat,
                        query,
                        near,
                        places,
                    });
            }
            Effect::SendStaticLocation {
                chat,
                point,
                local_id,
                reply_to,
                thread_root,
                ..
            } => {
                self.telegram
                    .send_location(chat, point, reply_to, thread_root)
                    .map_err(|error| self.scenario_error(error))?;
                self.handle_rich_media_ack(chat, local_id);
            }
            Effect::SendVenue {
                chat,
                venue,
                local_id,
                reply_to,
                thread_root,
                ..
            } => {
                self.telegram
                    .send_venue(chat, venue, reply_to, thread_root)
                    .map_err(|error| self.scenario_error(error))?;
                self.handle_rich_media_ack(chat, local_id);
            }
            _ => unreachable!("only location effects are routed here"),
        }
        Ok(())
    }
}
