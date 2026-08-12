impl Client {
    /// Exports a fresh QR-login token for this authorization key.
    pub async fn export_qr_login(&mut self) -> Result<QrLogin> {
        let mut restarts = 0;
        loop {
            let response = self
                .connection
                .invoke(&tl::functions::auth::ExportLoginToken {
                    api_id: self.credentials.api_id,
                    api_hash: self.credentials.api_hash.clone(),
                    except_ids: self.identity.iter().map(|identity| identity.id).collect(),
                })
                .await;
            match response {
                Ok(response) => return self.normalize_qr_login(response),
                Err(source) => match login_error_action(&source) {
                    LoginErrorAction::Restart if restarts < MAX_LOGIN_RESTARTS => {
                        restarts += 1;
                        compio::time::sleep(Duration::from_millis(250)).await;
                    }
                    LoginErrorAction::RequestPassword => {
                        return self
                            .begin_password_challenge()
                            .await
                            .map(QrLogin::PasswordRequired);
                    }
                    LoginErrorAction::Restart | LoginErrorAction::Propagate => {
                        return Err(Error::Invoke { source });
                    }
                },
            }
        }
    }

    /// Imports a QR-login token after Telegram requests data-center migration.
    pub async fn import_qr_login(&mut self, migration: QrLoginMigration) -> Result<QrLogin> {
        let response = self
            .connection
            .invoke(&tl::functions::auth::ImportLoginToken {
                token: migration.token,
            })
            .await;
        match response {
            Ok(response) => self.normalize_qr_login(response),
            Err(source) if login_error_action(&source) == LoginErrorAction::RequestPassword => self
                .begin_password_challenge()
                .await
                .map(QrLogin::PasswordRequired),
            Err(source) => Err(Error::Invoke { source }),
        }
    }

    /// Polls once for Telegram's `updateLoginToken` notification.
    ///
    /// The short delay keeps the server from being flooded while the poll RPC
    /// also drives the underlying `MTProto` receive loop.
    pub async fn poll_qr_login(&mut self) -> Result<bool> {
        if take_login_token_update(&mut self.connection) {
            return Ok(true);
        }
        compio::time::sleep(Duration::from_millis(500)).await;
        self.connection
            .invoke(&tl::functions::PingDelayDisconnect {
                ping_id: QR_PING_ID.fetch_add(1, Ordering::Relaxed),
                disconnect_delay: 30,
            })
            .await
            .context(InvokeSnafu)?;
        Ok(take_login_token_update(&mut self.connection))
    }

    fn normalize_qr_login(&mut self, response: tl::enums::auth::LoginToken) -> Result<QrLogin> {
        match response {
            tl::enums::auth::LoginToken::Token(token) => Ok(QrLogin::Pending(QrLoginToken {
                uri: qr_login_uri(&token.token),
                expires_at: token.expires,
            })),
            tl::enums::auth::LoginToken::MigrateTo(migration) => {
                Ok(QrLogin::Migrate(QrLoginMigration {
                    dc_id: migration.dc_id,
                    token: migration.token,
                }))
            }
            tl::enums::auth::LoginToken::Success(success) => {
                normalize_authorization(success.authorization).map(|identity| {
                    self.identity = Some(identity.clone());
                    QrLogin::Authorized(identity)
                })
            }
        }
    }
}
use super::*;
