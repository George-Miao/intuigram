//! Authorization-session lifecycle operations.

use super::*;

impl Client {
    /// Revokes this client's current Telegram authorization.
    pub async fn log_out(&mut self) -> Result<()> {
        self.connection
            .invoke(&tl::functions::auth::LogOut {})
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }
}
