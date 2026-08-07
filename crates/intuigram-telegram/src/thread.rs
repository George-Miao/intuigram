//! Telegram Thread request construction.

use grammers_tl_types as tl;
use intuigram_app::MessageId;

use crate::{Error, Result};

pub(super) fn read_request(
    peer: tl::enums::InputPeer,
    root: MessageId,
    max_id: MessageId,
) -> Result<tl::functions::messages::ReadDiscussion> {
    let msg_id =
        i32::try_from(root.0).map_err(|_| Error::InvalidMessageId { message_id: root.0 })?;
    let read_max_id = i32::try_from(max_id.0).map_err(|_| Error::InvalidMessageId {
        message_id: max_id.0,
    })?;
    Ok(tl::functions::messages::ReadDiscussion {
        peer,
        msg_id,
        read_max_id,
    })
}

#[cfg(test)]
mod tests {
    use grammers_tl_types as tl;
    use intuigram_app::MessageId;

    use super::read_request;

    #[test]
    fn read_discussion_keeps_thread_and_visible_message_distinct() {
        let request = read_request(
            tl::types::InputPeerUser {
                user_id: 7,
                access_hash: 11,
            }
            .into(),
            MessageId(40),
            MessageId(52),
        )
        .expect("valid Message IDs should construct a Thread read request");

        assert_eq!(request.msg_id, 40);
        assert_eq!(request.read_max_id, 52);
    }

    #[test]
    fn read_discussion_rejects_message_ids_outside_telegram_ints() {
        let error = read_request(
            tl::enums::InputPeer::PeerSelf,
            MessageId(i64::from(i32::MAX) + 1),
            MessageId(52),
        )
        .expect_err("oversized Message IDs must not be truncated");

        assert!(matches!(error, crate::Error::InvalidMessageId { .. }));
    }
}
