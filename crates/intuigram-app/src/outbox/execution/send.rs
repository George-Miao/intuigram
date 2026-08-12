use intuigram_lib::{ChatId, MediaCard, MediaKind, MessageId, PollOptionView, PollView};
use intuigram_telegram::InvocationPolicy;

use super::super::super::{Backend, derived_random_id};
use super::super::model::PreparedCommand;
use super::super::model::send::{MessageSend, Poll, TextMessage};
use super::message::{outgoing, success};
use super::{Result, Success, conversion, telegram};

pub(super) async fn text(
    backend: &mut Backend,
    command: &PreparedCommand,
    send: &MessageSend<TextMessage>,
    media: &[intuigram_store::OutboxMedia],
) -> Result<Success> {
    let destination = command.destination();
    let random_id = command
        .random_id()
        .expect("validated durable sends retain their random ID");
    let entities = conversion::entities(&send.content.entities);
    let server_id = if send.content.attachments.is_empty() {
        telegram(
            backend
                .client
                .send_text_with_policy(
                    intuigram_telegram::TextSend {
                        chat: ChatId(destination.chat_id),
                        text: send.content.text.clone(),
                        entities: entities.clone(),
                        link_preview: send.content.link_preview,
                        reply_to: send.reply_to.map(MessageId),
                        thread_root: destination.thread_root.map(MessageId),
                        monoforum_peer: destination.saved_peer.map(ChatId),
                        random_id,
                        schedule_date: None,
                    },
                    InvocationPolicy::SurfaceFloodWait,
                )
                .await,
        )?
    } else {
        send_attachments(backend, command, send, media, entities).await?
    };
    let message = outgoing(
        command,
        server_id,
        send.content.text.clone(),
        conversion::entities(&send.content.entities),
        send.reply_to,
    );
    success(command, server_id, message)
}

async fn send_attachments(
    backend: &mut Backend,
    command: &PreparedCommand,
    send: &MessageSend<TextMessage>,
    media: &[intuigram_store::OutboxMedia],
    mut entities: Vec<intuigram_lib::TextEntity>,
) -> Result<MessageId> {
    let destination = command.destination();
    let random_id = command
        .random_id()
        .expect("validated durable sends retain their random ID");
    let mut first = None;
    for (index, attachment) in send.content.attachments.iter().enumerate() {
        let upload = conversion::media(
            media,
            attachment.position,
            conversion::attachment_kind(attachment.kind),
        )?;
        let sent = telegram(
            backend
                .client
                .send_upload_with_policy(
                    intuigram_telegram::UploadSend {
                        chat: ChatId(destination.chat_id),
                        upload,
                        caption: if index == 0 {
                            send.content.text.clone()
                        } else {
                            String::new()
                        },
                        entities: if index == 0 {
                            std::mem::take(&mut entities)
                        } else {
                            Vec::new()
                        },
                        reply_to: send.reply_to.map(MessageId),
                        thread_root: destination.thread_root.map(MessageId),
                        monoforum_peer: destination.saved_peer.map(ChatId),
                        ids: intuigram_telegram::UploadIds {
                            file: derived_random_id(random_id, index, 0x4649_4c45),
                            message: derived_random_id(random_id, index, 0x4d45_5353),
                        },
                    },
                    InvocationPolicy::SurfaceFloodWait,
                )
                .await,
        )?;
        first.get_or_insert(sent);
    }
    Ok(first.expect("a nonempty prepared attachment list sends at least one Message"))
}

pub(super) async fn poll(
    backend: &mut Backend,
    command: &PreparedCommand,
    send: &MessageSend<Poll>,
) -> Result<Success> {
    let destination = command.destination();
    let server_id = telegram(
        backend
            .client
            .send_poll_with_policy(
                intuigram_telegram::PollSend {
                    chat: ChatId(destination.chat_id),
                    question: send.content.question.clone(),
                    options: send.content.options.clone(),
                    reply_to: send.reply_to.map(MessageId),
                    thread_root: destination.thread_root.map(MessageId),
                    monoforum_peer: destination.saved_peer.map(ChatId),
                    random_id: command
                        .random_id()
                        .expect("validated durable polls retain their random ID"),
                },
                InvocationPolicy::SurfaceFloodWait,
            )
            .await,
    )?;
    let mut message = outgoing(command, server_id, String::new(), Vec::new(), send.reply_to);
    message.details.media = Some(MediaCard {
        kind: MediaKind::Poll,
        title: "Poll".to_owned(),
        description: send.content.question.clone(),
        details: Vec::new(),
        poll: Some(PollView {
            quiz: false,
            multiple_choice: false,
            closed: false,
            total_voters: Some(0),
            options: send
                .content
                .options
                .iter()
                .map(|text| PollOptionView {
                    text: text.clone(),
                    voters: Some(0),
                    chosen: false,
                    correct: false,
                })
                .collect(),
            solution: None,
        }),
        specialized: None,
        remote_id: None,
    });
    success(command, server_id, message)
}
