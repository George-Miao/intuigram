use super::*;

const MAX_DIALOG_PAGE: i32 = 100;

pub(super) struct DialogBatch {
    pub(super) dialogs: Vec<tl::enums::Dialog>,
    pub(super) messages: Vec<tl::enums::Message>,
    pub(super) chats: Vec<tl::enums::Chat>,
    pub(super) users: Vec<tl::enums::User>,
}

impl Client {
    pub(super) async fn load_all_dialogs(&mut self, requested_page: i32) -> Result<DialogBatch> {
        let page_size = requested_page.clamp(1, MAX_DIALOG_PAGE);
        let mut request = tl::functions::messages::GetDialogs {
            exclude_pinned: false,
            folder_id: None,
            offset_date: 0,
            offset_id: 0,
            offset_peer: tl::enums::InputPeer::Empty,
            limit: page_size,
            hash: 0,
        };
        let mut batch = DialogBatch {
            dialogs: Vec::new(),
            messages: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        loop {
            let response = self
                .connection
                .invoke(&request)
                .await
                .context(InvokeSnafu)?;
            let (dialogs, messages, chats, users, complete) = match response {
                tl::enums::messages::Dialogs::Dialogs(page) => {
                    (page.dialogs, page.messages, page.chats, page.users, true)
                }
                tl::enums::messages::Dialogs::Slice(page) => {
                    let complete = page.dialogs.len() < page_size as usize;
                    (
                        page.dialogs,
                        page.messages,
                        page.chats,
                        page.users,
                        complete,
                    )
                }
                tl::enums::messages::Dialogs::NotModified(_) => {
                    return DialogsNotModifiedSnafu.fail();
                }
            };
            self.update_peer_cache(&chats, &users);
            let offset = if complete {
                None
            } else {
                dialog_offset(&dialogs, &messages, &self.peers)?
            };
            batch.dialogs.extend(dialogs);
            batch.messages.extend(messages);
            batch.chats.extend(chats);
            batch.users.extend(users);
            if complete {
                return Ok(batch);
            }
            let Some((offset_date, offset_id, offset_peer)) = offset else {
                return DialogOffsetUnavailableSnafu.fail();
            };
            request.exclude_pinned = true;
            request.offset_date = offset_date;
            request.offset_id = offset_id;
            request.offset_peer = offset_peer;
        }
    }
}

fn dialog_offset(
    dialogs: &[tl::enums::Dialog],
    messages: &[tl::enums::Message],
    peers: &PeerDirectory,
) -> Result<Option<(i32, i32, tl::enums::InputPeer)>> {
    let Some(dialog) = dialogs.iter().rev().find_map(|dialog| match dialog {
        tl::enums::Dialog::Dialog(dialog) => Some(dialog),
        tl::enums::Dialog::Folder(_) => None,
    }) else {
        return Ok(None);
    };
    let chat = marked_peer_id(&dialog.peer);
    let peer = peers.resolve(chat)?;
    let (date, id) = messages
        .iter()
        .find(|message| message_chat_id(message) == chat && message.id() == dialog.top_message)
        .and_then(message_offset)
        .unwrap_or((0, 0));
    Ok(Some((date, id, peer)))
}

fn message_offset(message: &tl::enums::Message) -> Option<(i32, i32)> {
    match message {
        tl::enums::Message::Empty(_) => None,
        tl::enums::Message::Message(message) => Some((message.date, message.id)),
        tl::enums::Message::Service(message) => Some((message.date, message.id)),
    }
}
