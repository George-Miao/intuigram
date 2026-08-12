use super::super::*;

pub(crate) fn seconds_until(expires_at: i32, server_time_offset: i32) -> u64 {
    let local_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let local_now = i64::try_from(local_now).unwrap_or(i64::MAX);
    seconds_until_at(expires_at, local_now, server_time_offset)
}

pub(crate) fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

pub(crate) fn seconds_until_at(expires_at: i32, local_now: i64, server_time_offset: i32) -> u64 {
    let server_now = local_now.saturating_add(i64::from(server_time_offset));
    u64::try_from(i64::from(expires_at).saturating_sub(server_now)).unwrap_or(0)
}
