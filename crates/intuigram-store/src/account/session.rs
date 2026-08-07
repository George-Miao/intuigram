pub(super) fn read_session(connection: &Connection) -> Result<Option<SessionMaterial>> {
    let row = connection
        .query_row(
            "SELECT dc_id, endpoint, auth_key, time_offset, first_salt FROM mtproto_session WHERE \
             singleton = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, i32>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .context(ReadSessionSnafu)?;
    row.map(|(dc_id, endpoint, key, time_offset, first_salt)| {
        let length = key.len();
        let auth_key = key
            .try_into()
            .map_err(|_| Error::InvalidAuthorizationKey { length })?;
        Ok(SessionMaterial::new(
            dc_id,
            endpoint,
            auth_key,
            time_offset,
            first_salt,
        ))
    })
    .transpose()
}
use super::*;
