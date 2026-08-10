use intuigram_lib::{AdapterEvent, AvatarRef, AvatarView};

use super::TestSystem;
use super::downloads::ONE_PIXEL_PNG;
use crate::error::Result;

impl TestSystem {
    pub(super) fn handle_avatar_load(&mut self, avatar: AvatarRef) -> Result<()> {
        self.telegram
            .load_avatar(avatar)
            .map_err(|error| self.scenario_error(error))?;
        let image = intuigram_media::decode_preview(ONE_PIXEL_PNG)
            .expect("the committed behavior PNG should decode");
        self.application
            .handle_adapter(AdapterEvent::AvatarReady(AvatarView { avatar, image }));
        Ok(())
    }
}
