use intuigram_lib::{
    AdapterEvent, ChatId, MessageId, RichMediaItemId, RichMediaItemView, RichMediaLibraryKind,
};

use super::TestSystem;

impl TestSystem {
    pub(super) fn handle_rich_media_browse(&mut self, kind: RichMediaLibraryKind) {
        self.application
            .handle_adapter(AdapterEvent::RichMediaLibraryReady {
                kind,
                items: vec![
                    RichMediaItemView {
                        id: RichMediaItemId(1),
                        label: "wave".to_owned(),
                    },
                    RichMediaItemView {
                        id: RichMediaItemId(2),
                        label: "party".to_owned(),
                    },
                ],
            });
    }

    pub(super) fn handle_rich_media_ack(&mut self, chat: ChatId, local_id: MessageId) {
        self.application
            .handle_adapter(AdapterEvent::RichMediaAcknowledged {
                chat,
                local_id,
                server_id: MessageId(1_000 - local_id.0),
            });
    }
}
