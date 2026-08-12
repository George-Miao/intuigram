use intuigram_store::SyncCursor;
use intuigram_telegram::UpdateCursor;

use super::{Result, UpdateGapSnafu};

/// Converts Telegram's optional cursor components into the durable Account
/// scope.
#[must_use]
pub fn store_cursor(cursor: UpdateCursor) -> SyncCursor {
    SyncCursor {
        scope: cursor.scope.storage_key(),
        pts: cursor.pts.unwrap_or(0),
        qts: cursor.qts.unwrap_or(0),
        date: cursor.date.unwrap_or(0),
        seq: cursor.seq.unwrap_or(0),
    }
}

pub(super) enum CursorDelta {
    Applied {
        cursor: SyncCursor,
        expose_events: bool,
    },
    Deferred {
        scope: String,
    },
}

pub(super) fn apply_cursor_delta(
    mut cursor: SyncCursor,
    delta: &UpdateCursor,
) -> Result<CursorDelta> {
    if delta.gap {
        return UpdateGapSnafu {
            scope: cursor.scope,
        }
        .fail();
    }
    let mut expose_events = false;
    if let Some(pts) = delta.pts {
        let expected = cursor.pts.saturating_add(delta.pts_count);
        if cursor.pts != 0 && pts > cursor.pts && pts != expected {
            return Ok(CursorDelta::Deferred {
                scope: cursor.scope,
            });
        }
        expose_events |=
            pts > cursor.pts || (cursor.pts != 0 && pts == cursor.pts && delta.pts_count == 0);
        cursor.pts = cursor.pts.max(pts);
    }
    if let Some(qts) = delta.qts {
        expose_events |= qts > cursor.qts;
        cursor.qts = cursor.qts.max(qts);
    }
    if let Some(date) = delta.date {
        expose_events |= date > cursor.date;
        cursor.date = cursor.date.max(date);
    }
    if let Some(seq) = delta.seq {
        if cursor.seq != 0
            && delta
                .seq_start
                .is_some_and(|start| start > cursor.seq.saturating_add(1))
        {
            return Ok(CursorDelta::Deferred {
                scope: cursor.scope,
            });
        }
        expose_events |= seq > cursor.seq;
        cursor.seq = cursor.seq.max(seq);
    }
    Ok(CursorDelta::Applied {
        cursor,
        expose_events,
    })
}
