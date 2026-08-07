use super::*;

pub(super) fn updates_cursors(updates: &tl::enums::Updates) -> Vec<UpdateCursor> {
    match updates {
        tl::enums::Updates::TooLong => vec![UpdateCursor {
            gap: true,
            ..UpdateCursor::default()
        }],
        tl::enums::Updates::UpdateShortMessage(update) => {
            vec![account_pts(update.pts, update.pts_count, Some(update.date))]
        }
        tl::enums::Updates::UpdateShortChatMessage(update) => {
            vec![account_pts(update.pts, update.pts_count, Some(update.date))]
        }
        tl::enums::Updates::UpdateShortSentMessage(update) => {
            vec![account_pts(update.pts, update.pts_count, Some(update.date))]
        }
        tl::enums::Updates::UpdateShort(update) => {
            let mut cursors = update_cursors(&update.update);
            merge_cursor(
                &mut cursors,
                UpdateCursor {
                    date: Some(update.date),
                    ..UpdateCursor::default()
                },
            );
            cursors
        }
        tl::enums::Updates::Combined(updates) => envelope_cursors(
            &updates.updates,
            updates.date,
            updates.seq_start,
            updates.seq,
        ),
        tl::enums::Updates::Updates(updates) => {
            envelope_cursors(&updates.updates, updates.date, updates.seq, updates.seq)
        }
    }
}

pub(super) fn update_cursors(update: &tl::enums::Update) -> Vec<UpdateCursor> {
    let cursor = match update {
        tl::enums::Update::NewMessage(update) => account_pts(update.pts, update.pts_count, None),
        tl::enums::Update::EditMessage(update) => account_pts(update.pts, update.pts_count, None),
        tl::enums::Update::DeleteMessages(update) => {
            account_pts(update.pts, update.pts_count, None)
        }
        tl::enums::Update::ReadHistoryInbox(update) => {
            account_pts(update.pts, update.pts_count, None)
        }
        tl::enums::Update::ReadHistoryOutbox(update) => {
            account_pts(update.pts, update.pts_count, None)
        }
        tl::enums::Update::ReadMessagesContents(update) => {
            account_pts(update.pts, update.pts_count, update.date)
        }
        tl::enums::Update::FolderPeers(update) => account_pts(update.pts, update.pts_count, None),
        tl::enums::Update::PinnedMessages(update) => {
            account_pts(update.pts, update.pts_count, None)
        }
        tl::enums::Update::NewChannelMessage(update) => {
            channel_message_cursor(&update.message, update.pts, update.pts_count)
        }
        tl::enums::Update::EditChannelMessage(update) => {
            channel_message_cursor(&update.message, update.pts, update.pts_count)
        }
        tl::enums::Update::DeleteChannelMessages(update) => {
            channel_pts(update.channel_id, update.pts, update.pts_count)
        }
        tl::enums::Update::ReadChannelInbox(update) => {
            channel_pts(update.channel_id, update.pts, 0)
        }
        tl::enums::Update::PinnedChannelMessages(update) => {
            channel_pts(update.channel_id, update.pts, update.pts_count)
        }
        tl::enums::Update::ChannelTooLong(update) => UpdateCursor {
            scope: channel_scope(update.channel_id),
            pts: update.pts,
            gap: true,
            ..UpdateCursor::default()
        },
        tl::enums::Update::PtsChanged => UpdateCursor {
            gap: true,
            ..UpdateCursor::default()
        },
        tl::enums::Update::NewEncryptedMessage(update) => UpdateCursor {
            qts: Some(update.qts),
            ..UpdateCursor::default()
        },
        _ => return Vec::new(),
    };
    vec![cursor]
}

fn envelope_cursors(
    updates: &[tl::enums::Update],
    date: i32,
    seq_start: i32,
    seq: i32,
) -> Vec<UpdateCursor> {
    let mut cursors = vec![UpdateCursor {
        date: Some(date),
        seq: Some(seq),
        seq_start: Some(seq_start),
        ..UpdateCursor::default()
    }];
    for update in updates {
        for cursor in update_cursors(update) {
            merge_cursor(&mut cursors, cursor);
        }
    }
    cursors
}

fn merge_cursor(cursors: &mut Vec<UpdateCursor>, next: UpdateCursor) {
    let Some(current) = cursors.iter_mut().find(|cursor| cursor.scope == next.scope) else {
        cursors.push(next);
        return;
    };
    let had_pts = current.pts.is_some();
    if let (Some(previous), Some(next_pts)) = (current.pts, next.pts)
        && next.pts_count > 0
        && previous.saturating_add(next.pts_count) != next_pts
    {
        current.gap = true;
    }
    current.pts = next.pts.or(current.pts);
    current.pts_count = if !had_pts {
        next.pts_count
    } else if current.pts_count == 0 || next.pts_count == 0 {
        0
    } else {
        current.pts_count.saturating_add(next.pts_count)
    };
    current.qts = next.qts.or(current.qts);
    current.date = next.date.or(current.date);
    current.seq = next.seq.or(current.seq);
    current.seq_start = next.seq_start.or(current.seq_start);
    current.gap |= next.gap;
}

fn account_pts(pts: i32, pts_count: i32, date: Option<i32>) -> UpdateCursor {
    UpdateCursor {
        pts: Some(pts),
        pts_count,
        date,
        ..UpdateCursor::default()
    }
}

fn channel_message_cursor(message: &tl::enums::Message, pts: i32, pts_count: i32) -> UpdateCursor {
    UpdateCursor {
        scope: UpdateScope::Channel(message_chat_id(message)),
        pts: Some(pts),
        pts_count,
        ..UpdateCursor::default()
    }
}

fn channel_pts(channel_id: i64, pts: i32, pts_count: i32) -> UpdateCursor {
    UpdateCursor {
        scope: channel_scope(channel_id),
        pts: Some(pts),
        pts_count,
        ..UpdateCursor::default()
    }
}

fn channel_scope(channel_id: i64) -> UpdateScope {
    UpdateScope::Channel(ChatId(mark_channel_id(channel_id)))
}
