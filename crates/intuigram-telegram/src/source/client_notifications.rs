use super::*;

impl Client {
    pub(super) async fn notification_defaults(
        &mut self,
        unix_time: i64,
    ) -> Result<NotificationDefaults> {
        let users = self
            .notification_default(tl::enums::InputNotifyPeer::InputNotifyUsers, unix_time)
            .await?;
        let chats = self
            .notification_default(tl::enums::InputNotifyPeer::InputNotifyChats, unix_time)
            .await?;
        let broadcasts = self
            .notification_default(tl::enums::InputNotifyPeer::InputNotifyBroadcasts, unix_time)
            .await?;
        Ok(NotificationDefaults::new(users, chats, broadcasts))
    }

    async fn notification_default(
        &mut self,
        peer: tl::enums::InputNotifyPeer,
        unix_time: i64,
    ) -> Result<bool> {
        let settings = self
            .connection
            .invoke(&tl::functions::account::GetNotifySettings { peer })
            .await
            .context(InvokeSnafu)?;
        Ok(notifications_muted_at(&settings, unix_time, false))
    }
}
