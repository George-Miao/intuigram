use super::*;

pub(super) fn affected_messages_cursor(
    bytes: &[u8],
    request: Option<&[u8]>,
) -> Option<UpdateCursor> {
    let tl::enums::messages::AffectedMessages::Messages(affected) =
        tl::enums::messages::AffectedMessages::from_bytes(bytes).ok()?;
    Some(UpdateCursor {
        scope: affected_messages_scope(request),
        pts: Some(affected.pts),
        pts_count: affected.pts_count,
        ..UpdateCursor::default()
    })
}

fn affected_messages_scope(request: Option<&[u8]>) -> UpdateScope {
    request
        .and_then(channel_delete_request)
        .map_or(UpdateScope::Account, |channel_id| {
            UpdateScope::Channel(ChatId(mark_channel_id(channel_id)))
        })
}

fn channel_delete_request(request: &[u8]) -> Option<i64> {
    let constructor = u32::from_bytes(request).ok()?;
    if constructor != tl::functions::channels::DeleteMessages::CONSTRUCTOR_ID {
        return None;
    }
    let body = request.get(size_of::<u32>()..)?;
    let request = tl::functions::channels::DeleteMessages::deserialize(
        &mut tl::deserialize::Cursor::from_slice(body),
    )
    .ok()?;
    match request.channel {
        tl::enums::InputChannel::Channel(channel) => Some(channel.channel_id),
        tl::enums::InputChannel::Empty | tl::enums::InputChannel::FromMessage(_) => None,
    }
}
