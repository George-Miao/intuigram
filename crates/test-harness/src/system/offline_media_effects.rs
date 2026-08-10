use intuigram_lib::{AdapterEvent, Effect, OfflineMediaPolicy, OfflineMediaTarget};
use snafu::ResultExt;

use super::TestSystem;
use crate::error::{Result, StoreSnafu};

impl TestSystem {
    pub(super) fn handle_offline_media_effect(&mut self, effect: Effect) -> Result<()> {
        match effect {
            Effect::SetChatMediaOffline(policy) => self.handle_offline_media_policy(policy),
            Effect::CacheMediaOffline(target) => {
                self.handle_offline_media_cache(target);
                Ok(())
            }
            _ => unreachable!("only offline-media effects reach the offline-media handler"),
        }
    }

    pub(super) fn handle_offline_media_policy(&mut self, policy: OfflineMediaPolicy) -> Result<()> {
        self.database
            .set_chat_media_offline(policy.chat.0, policy.keep)
            .context(StoreSnafu)?;
        self.application
            .handle_adapter(AdapterEvent::ChatMediaOfflineChanged(policy));
        Ok(())
    }

    pub(super) fn handle_offline_media_cache(&mut self, target: OfflineMediaTarget) {
        self.application
            .handle_adapter(AdapterEvent::MediaCachedOffline(target));
    }
}
