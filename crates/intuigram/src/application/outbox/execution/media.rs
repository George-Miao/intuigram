use intuigram_app::{ChatId, MessageId};
use intuigram_telegram::InvocationPolicy;

use super::super::super::Backend;
use super::super::model::PreparedCommand;
use super::super::model::send::{Contact, LibraryMedia, MessageSend};
use super::super::model::shared::PreparedMedia;
use super::message::{outgoing, success};
use super::{Result, conversion, telegram};

pub(super) async fn library(
    backend: &mut Backend,
    command: &PreparedCommand,
    send: &MessageSend<LibraryMedia>,
) -> Result<super::Success> {
    let content = &send.content;
    let kind = conversion::library_kind(content.kind);
    let label = format!("[{kind:?}]");
    let entry = intuigram_telegram::MediaLibraryEntry::from_remote_parts(
        content.document_id,
        label.clone(),
        kind,
        content.access_hash,
        content.file_reference.clone(),
    );
    let destination = command.destination();
    let server_id = telegram(
        backend
            .client
            .send_library_media_with_policy(
                intuigram_telegram::LibraryMediaSend {
                    chat: ChatId(destination.chat_id),
                    entry,
                    reply_to: send.reply_to.map(MessageId),
                    thread_root: destination.thread_root.map(MessageId),
                    monoforum_peer: destination.saved_peer.map(ChatId),
                    random_id: random_id(command),
                },
                InvocationPolicy::SurfaceFloodWait,
            )
            .await,
    )?;
    success(
        command,
        server_id,
        outgoing(command, server_id, label, Vec::new(), send.reply_to),
    )
}

pub(super) async fn contact(
    backend: &mut Backend,
    command: &PreparedCommand,
    send: &MessageSend<Contact>,
) -> Result<super::Success> {
    let destination = command.destination();
    let server_id = telegram(
        backend
            .client
            .send_contact_with_policy(
                intuigram_telegram::ContactCardSend {
                    chat: ChatId(destination.chat_id),
                    phone_number: send.content.phone.clone(),
                    first_name: send.content.first_name.clone(),
                    last_name: send.content.last_name.clone(),
                    reply_to: send.reply_to.map(MessageId),
                    thread_root: destination.thread_root.map(MessageId),
                    monoforum_peer: destination.saved_peer.map(ChatId),
                    random_id: random_id(command),
                },
                InvocationPolicy::SurfaceFloodWait,
            )
            .await,
    )?;
    let body = format!(
        "[Contact] {} {}",
        send.content.first_name, send.content.last_name
    );
    success(
        command,
        server_id,
        outgoing(command, server_id, body, Vec::new(), send.reply_to),
    )
}

pub(super) async fn upload(
    backend: &mut Backend,
    command: &PreparedCommand,
    send: &MessageSend<PreparedMedia>,
    media: &[intuigram_store::OutboxMedia],
) -> Result<super::Success> {
    let destination = command.destination();
    let upload = conversion::media(
        media,
        send.content.position,
        conversion::upload_kind(send.content.kind),
    )?;
    let body = format!("[{:?}] {}", upload.kind, upload.name);
    let random_id = random_id(command);
    let server_id = telegram(
        backend
            .client
            .send_upload_with_policy(
                intuigram_telegram::UploadSend {
                    chat: ChatId(destination.chat_id),
                    upload,
                    caption: String::new(),
                    entities: Vec::new(),
                    reply_to: send.reply_to.map(MessageId),
                    thread_root: destination.thread_root.map(MessageId),
                    monoforum_peer: destination.saved_peer.map(ChatId),
                    ids: intuigram_telegram::UploadIds {
                        file: super::super::super::derived_random_id(random_id, 0, 0x4649_4c45),
                        message: super::super::super::derived_random_id(random_id, 0, 0x4d45_5353),
                    },
                },
                InvocationPolicy::SurfaceFloodWait,
            )
            .await,
    )?;
    success(
        command,
        server_id,
        outgoing(command, server_id, body, Vec::new(), send.reply_to),
    )
}

fn random_id(command: &PreparedCommand) -> i64 {
    command
        .random_id()
        .expect("validated durable media sends retain their random ID")
}
