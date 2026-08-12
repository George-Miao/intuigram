use super::super::*;

pub(crate) async fn request_code_with_migration(
    credentials: &ApplicationCredentials,
    pending: &AccountDatabase,
    client: &mut Client,
    session: &mut Session,
    phone_number: &str,
) -> Result<CodeRequest> {
    loop {
        match client.request_login_code(phone_number.to_owned()).await {
            Ok(request) => return Ok(request),
            Err(error) => {
                let Some(dc_id) = error.phone_migration_dc() else {
                    return Err(Error::Telegram { source: error });
                };
                let endpoint = client
                    .data_center_endpoint(dc_id)
                    .context(MissingDataCenterSnafu { dc_id })?;
                let route = client.connection_route();
                let connected = Client::connect_new(dc_id, endpoint, credentials.clone(), route)
                    .await
                    .context(TelegramSnafu)?;
                *client = connected.0;
                *session = connected.1;
                pending
                    .save_session(store_session(session))
                    .context(AccountDatabaseSnafu)?;
            }
        }
    }
}
