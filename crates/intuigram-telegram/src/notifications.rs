use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NotificationDefaults {
    users: bool,
    chats: bool,
    broadcasts: bool,
}

impl NotificationDefaults {
    pub(crate) const fn new(users: bool, chats: bool, broadcasts: bool) -> Self {
        Self {
            users,
            chats,
            broadcasts,
        }
    }

    pub(crate) const fn muted(self, kind: ChatKind) -> bool {
        match kind {
            ChatKind::SavedMessages | ChatKind::Private | ChatKind::Bot => self.users,
            ChatKind::BasicGroup | ChatKind::Supergroup | ChatKind::Gigagroup => self.chats,
            ChatKind::Channel => self.broadcasts,
            ChatKind::Inaccessible => false,
        }
    }
}

pub(crate) fn notifications_muted_at(
    settings: &tl::enums::PeerNotifySettings,
    unix_time: i64,
    inherited: bool,
) -> bool {
    let tl::enums::PeerNotifySettings::Settings(settings) = settings;
    settings
        .mute_until
        .map_or(inherited, |until| i64::from(until) > unix_time)
}
