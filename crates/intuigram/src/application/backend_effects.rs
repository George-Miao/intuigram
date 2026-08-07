impl Backend {
    pub(super) async fn execute(&mut self, effect: AdapterEffect) -> Result<Option<AdapterEvent>> {
        let AdapterEffect { effect, random_id } = effect;
        match effect {
            Effect::Quit | Effect::Reconnect => Ok(None),
            Effect::SetChatFolder {
                chat,
                folder,
                included,
            } => Ok(Some(
                match self.client.set_chat_folder(chat, folder, included).await {
                    Ok(()) => AdapterEvent::FolderMembershipChanged {
                        chat,
                        folder,
                        included,
                    },
                    Err(source) if source.is_connection_failure() => {
                        return Err(Error::Telegram { source });
                    }
                    Err(error) => AdapterEvent::OperationFailed(error.to_string()),
                },
            )),
            Effect::LoadChat { chat, selection } => self.load_selected_chat(chat, selection).await,
            Effect::SaveSelection {
                folder,
                chat,
                message,
            } => {
                self.save_selection(folder, chat, message).await?;
                Ok(None)
            }
            Effect::LoadThread { chat, root } => match self.load_thread(chat, root).await {
                Ok(messages) => Ok(Some(AdapterEvent::ThreadLoaded {
                    chat,
                    root,
                    messages,
                })),
                Err(error) => history_failure_event(chat, Some(root), error),
            },
            Effect::ReadThread { chat, root, max_id } => {
                match self.client.read_thread(chat, root, max_id).await {
                    Ok(()) => Ok(None),
                    Err(source) if source.is_connection_failure() => {
                        Err(Error::Telegram { source })
                    }
                    Err(error) => Ok(Some(AdapterEvent::OperationFailed(error.to_string()))),
                }
            }
            Effect::ReadClipboard { chat, thread_root } => {
                Ok(Some(match self.read_clipboard(chat, thread_root).await {
                    Ok(event) => event,
                    Err(error) => AdapterEvent::OperationFailed(error.to_string()),
                }))
            }
            Effect::SaveDraft {
                chat,
                thread_root,
                text,
                reply_to,
            } => {
                self.save_draft(chat, thread_root, text, reply_to).await?;
                Ok(None)
            }
            Effect::SendMessage {
                chat,
                text,
                entities,
                link_preview,
                reply_to,
                thread_root,
                attachments,
                local_id,
            } => {
                self.persist_outgoing(OutgoingRecord {
                    chat,
                    local_id,
                    text: &text,
                    entities: &entities,
                    reply_to,
                    thread_root,
                    delivery: DeliveryState::Pending,
                })
                .await?;
                self.save_draft(chat, thread_root, String::new(), None)
                    .await?;
                let result = self
                    .send_message(MessageSend {
                        chat,
                        text: text.clone(),
                        entities: entities.clone(),
                        link_preview,
                        reply_to,
                        thread_root,
                        attachment_ids: attachments,
                        random_id: random_id.expect("every queued send has an idempotency token"),
                    })
                    .await;
                let result = match result {
                    Err(Error::Telegram { source }) if source.is_connection_failure() => {
                        return Err(Error::Telegram { source });
                    }
                    result => result,
                };
                self.persist_outgoing(OutgoingRecord {
                    chat,
                    local_id,
                    text: &text,
                    entities: &entities,
                    reply_to,
                    thread_root,
                    delivery: if result.is_ok() {
                        DeliveryState::Sent
                    } else {
                        DeliveryState::Failed
                    },
                })
                .await?;
                Ok(Some(match result {
                    Ok(_) => AdapterEvent::MessageAcknowledged { chat, local_id },
                    Err(error) => AdapterEvent::MessageFailed {
                        chat,
                        local_id,
                        thread_root,
                        text,
                        reason: error.to_string(),
                    },
                }))
            }
            Effect::SendPoll {
                chat,
                question,
                options,
                reply_to,
                thread_root,
                local_id,
            } => {
                self.persist_poll(PollPersistence {
                    chat,
                    local_id,
                    question: &question,
                    options: &options,
                    reply_to,
                    thread_root,
                    delivery: DeliveryState::Pending,
                })
                .await?;
                let result = self
                    .client
                    .send_poll(
                        chat,
                        question.clone(),
                        options.clone(),
                        reply_to,
                        thread_root,
                        random_id.expect("every queued poll has an idempotency token"),
                    )
                    .await;
                let result = match result {
                    Err(source) if source.is_connection_failure() => {
                        return Err(Error::Telegram { source });
                    }
                    result => result,
                };
                self.persist_poll(PollPersistence {
                    chat,
                    local_id,
                    question: &question,
                    options: &options,
                    reply_to,
                    thread_root,
                    delivery: if result.is_ok() {
                        DeliveryState::Sent
                    } else {
                        DeliveryState::Failed
                    },
                })
                .await?;
                Ok(Some(match result {
                    Ok(()) => AdapterEvent::MessageAcknowledged { chat, local_id },
                    Err(error) => AdapterEvent::PollFailed {
                        chat,
                        local_id,
                        thread_root,
                        text: std::iter::once(question)
                            .chain(options)
                            .collect::<Vec<_>>()
                            .join("\n"),
                        reason: error.to_string(),
                    },
                }))
            }
            Effect::EditMessage {
                chat,
                message,
                draft_text,
            } => {
                let message = *message;
                let result = self
                    .client
                    .edit_text(
                        chat,
                        message.id,
                        message.body.clone(),
                        message.details.entities.clone(),
                    )
                    .await;
                match result {
                    Ok(()) => {
                        self.store
                            .save_messages(vec![encode_stored_message(chat, &message)])
                            .context(AccountDatabaseSnafu)?
                            .await
                            .context(AccountDatabaseSnafu)?;
                        Ok(Some(AdapterEvent::MessageUpdated {
                            chat,
                            message: Box::new(message),
                        }))
                    }
                    Err(source) if source.is_connection_failure() => {
                        Err(Error::Telegram { source })
                    }
                    Err(error) => Ok(Some(AdapterEvent::MessageEditFailed {
                        chat,
                        message: message.id,
                        text: draft_text,
                        reason: error.to_string(),
                    })),
                }
            }
            Effect::DeleteMessages { chat, messages } => {
                let result = self.client.delete_messages(chat, messages.clone()).await;
                match result {
                    Ok(()) => {
                        self.store
                            .delete_messages(
                                Some(chat.0),
                                messages.iter().map(|message| message.0).collect(),
                            )
                            .context(AccountDatabaseSnafu)?
                            .await
                            .context(AccountDatabaseSnafu)?;
                        Ok(Some(AdapterEvent::MessagesDeleted {
                            chat: Some(chat),
                            ids: messages,
                        }))
                    }
                    Err(source) if source.is_connection_failure() => {
                        Err(Error::Telegram { source })
                    }
                    Err(error) => Ok(Some(AdapterEvent::OperationFailed(error.to_string()))),
                }
            }
            Effect::ForwardMessage {
                source,
                destination,
                message,
            } => {
                let result = self
                    .client
                    .forward_message(
                        source,
                        destination,
                        message,
                        random_id.expect("every queued forward has an idempotency token"),
                    )
                    .await;
                match result {
                    Ok(()) => Ok(None),
                    Err(source) if source.is_connection_failure() => {
                        Err(Error::Telegram { source })
                    }
                    Err(error) => Ok(Some(AdapterEvent::OperationFailed(error.to_string()))),
                }
            }
            Effect::ReactMessage {
                chat,
                message,
                reaction,
            } => {
                let message = *message;
                let result = self.client.react_message(chat, message.id, reaction).await;
                match result {
                    Ok(()) => {
                        self.store
                            .save_messages(vec![encode_stored_message(chat, &message)])
                            .context(AccountDatabaseSnafu)?
                            .await
                            .context(AccountDatabaseSnafu)?;
                        Ok(Some(AdapterEvent::MessageUpdated {
                            chat,
                            message: Box::new(message),
                        }))
                    }
                    Err(source) if source.is_connection_failure() => {
                        Err(Error::Telegram { source })
                    }
                    Err(error) => Ok(Some(AdapterEvent::OperationFailed(error.to_string()))),
                }
            }
            Effect::SetMessagePinned {
                chat: _,
                message: _,
                pinned: _,
            } => MisroutedPinEffectSnafu.fail(),
            Effect::VotePoll {
                chat,
                message,
                options,
            } => {
                let mut message = *message;
                let result = self.client.vote_poll(chat, message.id, options).await;
                match result {
                    Ok(media) => {
                        message.details.media = Some(media);
                        self.store
                            .save_messages(vec![encode_stored_message(chat, &message)])
                            .context(AccountDatabaseSnafu)?
                            .await
                            .context(AccountDatabaseSnafu)?;
                        Ok(Some(AdapterEvent::MessageUpdated {
                            chat,
                            message: Box::new(message),
                        }))
                    }
                    Err(source) if source.is_connection_failure() => {
                        Err(Error::Telegram { source })
                    }
                    Err(error) => Ok(Some(AdapterEvent::OperationFailed(error.to_string()))),
                }
            }
            Effect::OpenExternalLink { url } => Ok(Some(
                match intuigram_media::PlatformLauncher.open_url(&url).await {
                    Ok(()) => AdapterEvent::OperationCompleted(format!("Opened {url}")),
                    Err(error) => AdapterEvent::OperationFailed(error.to_string()),
                },
            )),
            Effect::ResolveTelegramUsername { username } => {
                Ok(Some(match self.client.resolve_username(username).await {
                    Ok(chat) => AdapterEvent::TelegramLinkResolved { chat },
                    Err(source) if source.is_connection_failure() => {
                        return Err(Error::Telegram { source });
                    }
                    Err(error) => AdapterEvent::OperationFailed(error.to_string()),
                }))
            }
            Effect::LoadMediaPreview { chat, message } => {
                Ok(Some(match self.load_media_preview(chat, message).await {
                    Ok(Some(image)) => AdapterEvent::MediaPreviewReady(MediaPreviewView {
                        chat,
                        message,
                        image,
                    }),
                    Ok(None) => AdapterEvent::MediaPreviewFailed { chat, message },
                    Err(Error::Telegram { source }) if source.is_connection_failure() => {
                        return Err(Error::Telegram { source });
                    }
                    Err(_) => AdapterEvent::MediaPreviewFailed { chat, message },
                }))
            }
            Effect::DownloadMedia {
                chat,
                message,
                destination,
            } => Ok(Some(
                match self.download_media(chat, message, destination).await {
                    Ok(download) => AdapterEvent::DownloadReady { chat, download },
                    Err(Error::Telegram { source }) if source.is_connection_failure() => {
                        return Err(Error::Telegram { source });
                    }
                    Err(error) => AdapterEvent::OperationFailed(error.to_string()),
                },
            )),
            Effect::OpenDownload { download, reveal } => {
                let path = self.downloaded.paths.get(&download).cloned().ok_or(
                    Error::DownloadUnavailable {
                        download_id: download.0,
                    },
                );
                Ok(Some(match path {
                    Ok(path) => {
                        let result = if reveal {
                            intuigram_media::PlatformLauncher.reveal_file(&path).await
                        } else {
                            intuigram_media::PlatformLauncher.open_file(&path).await
                        };
                        match result {
                            Ok(()) => AdapterEvent::OperationCompleted(if reveal {
                                format!("Revealed {}", path.display())
                            } else {
                                format!("Opened {}", path.display())
                            }),
                            Err(error) => AdapterEvent::OperationFailed(error.to_string()),
                        }
                    }
                    Err(error) => AdapterEvent::OperationFailed(error.to_string()),
                }))
            }
        }
    }
}
use super::*;
