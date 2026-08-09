use compio::runtime::ResumeUnwind;
use intuigram_app::OfflineMediaFailure;

use super::*;

impl Backend {
    pub(in crate::application) async fn execute(
        &mut self,
        effect: AdapterEffect,
    ) -> Result<Option<AdapterEvent>> {
        let AdapterEffect { effect, random_id } = effect;
        match effect {
            effect @ (Effect::SearchPlaces { .. }
            | Effect::SendStaticLocation { .. }
            | Effect::SendVenue { .. }) => self.execute_location(effect, random_id).await.map(Some),
            effect @ (Effect::BrowseRichMedia { .. }
            | Effect::SendLibraryMedia { .. }
            | Effect::SendRichMediaFile { .. }
            | Effect::RecordRichMedia { .. }
            | Effect::SendContact { .. }) => {
                self.execute_rich_media(effect, random_id).await.map(Some)
            }
            effect @ (Effect::LoadScheduledMessages { .. } | Effect::ScheduledOperation { .. }) => {
                self.execute_scheduled(effect, random_id).await.map(Some)
            }
            Effect::FolderOperation { operation } => {
                self.execute_folder_operation(operation).await.map(Some)
            }
            Effect::RefreshFolders => self.refresh_folders().await.map(Some),
            Effect::AccountLifecycle { request } => {
                if matches!(request, AccountLifecycle::Logout(_)) {
                    return Ok(Some(match self.client.log_out().await {
                        Ok(()) => AdapterEvent::AccountLifecycleReady(request),
                        Err(error) => AdapterEvent::OperationFailed(format!(
                            "Telegram did not confirm logout; local Account data was preserved: \
                             {error}"
                        )),
                    }));
                }
                Ok(Some(AdapterEvent::AccountLifecycleReady(request)))
            }
            Effect::Notify { .. } => {
                compio::runtime::spawn_blocking(|| -> io::Result<()> {
                    let mut stderr = io::stderr().lock();
                    stderr.write_all(b"\x07")?;
                    stderr.flush()
                })
                .await
                .resume_unwind()
                .expect("an awaited terminal-bell task cannot be cancelled")
                .context(NotificationSnafu)?;
                Ok(None)
            }
            Effect::Quit | Effect::Reconnect => Ok(None),
            Effect::ResolveOutbox { item, action } => self.resolve_outbox(item, action).await,
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
            Effect::SetChatMediaOffline(_) => {
                unreachable!("offline-media policy persistence is a local ordered effect")
            }
            Effect::CacheMediaOffline(target) => {
                Ok(Some(match self.cache_media_offline(target).await {
                    Ok(()) => AdapterEvent::MediaCachedOffline(target),
                    Err(Error::Telegram { source }) if source.is_connection_failure() => {
                        return Err(Error::Telegram { source });
                    }
                    Err(error) => AdapterEvent::MediaCacheOfflineFailed(OfflineMediaFailure {
                        chat: target.chat,
                        message: Some(target.message),
                        reason: error.to_string(),
                    }),
                }))
            }
            Effect::LoadChat {
                chat,
                selection,
                transcript_anchors,
            } => {
                self.load_selected_chat(chat, selection, transcript_anchors)
                    .await
            }
            Effect::SaveSelection {
                folder,
                chat,
                message,
                transcript_anchors,
            } => {
                self.save_selection(folder, chat, message, transcript_anchors)
                    .await?;
                Ok(None)
            }
            Effect::LoadThread {
                chat,
                root,
                saved_peer,
            } => match self.load_thread(chat, root).await {
                Ok(messages) => Ok(Some(AdapterEvent::ThreadLoaded {
                    chat,
                    root,
                    saved_peer,
                    messages,
                })),
                Err(error) => history_failure_event(chat, Some(root), saved_peer, error),
            },
            Effect::LoadTopics(chat) => self.load_topics(chat).await,
            Effect::LoadSavedDialogs(chat) => self.load_saved_dialogs(chat).await,
            Effect::LoadSavedHistory { chat, peer } => self.load_saved_history(chat, peer).await,
            Effect::ReadThread {
                chat, root, max_id, ..
            } => self.execute_thread_read(chat, root, max_id).await,
            Effect::ReadHistory {
                chat,
                max_id,
                saved_peer,
            } => self.execute_history_read(chat, saved_peer, max_id).await,
            Effect::ReadClipboard {
                chat,
                thread_root,
                saved_peer,
            } => Ok(Some(
                match self.read_clipboard(chat, thread_root, saved_peer).await {
                    Ok(event) => event,
                    Err(error) => AdapterEvent::OperationFailed(error.to_string()),
                },
            )),
            Effect::PickAttachment {
                chat,
                thread_root,
                saved_peer,
            } => Ok(Some(AdapterEvent::AttachmentPathRequired {
                chat,
                thread_root,
                saved_peer,
            })),
            Effect::SelectAttachment {
                chat,
                thread_root,
                saved_peer,
                path,
            } => {
                let path = PathBuf::from(path);
                let metadata = compio::fs::metadata(&path)
                    .await
                    .context(ReadAttachmentSnafu { path: path.clone() });
                Ok(Some(match metadata {
                    Ok(metadata) if metadata.is_file() => {
                        let mime_type = mime_type_for_path(&path);
                        let kind = if mime_type.starts_with("image/") {
                            AttachmentKind::Photo
                        } else if mime_type.starts_with("video/") {
                            AttachmentKind::Video
                        } else {
                            AttachmentKind::File
                        };
                        let name = path.file_name().map_or_else(
                            || "attachment".to_owned(),
                            |name| name.to_string_lossy().into_owned(),
                        );
                        let id = self
                            .attachments
                            .register(AttachmentPayload::File { path, kind });
                        AdapterEvent::ClipboardReady {
                            chat,
                            thread_root,
                            saved_peer,
                            text: None,
                            attachments: vec![AttachmentView { id, kind, name }],
                        }
                    }
                    Ok(_) => AdapterEvent::OperationFailed(
                        "Attachment path must identify a regular file".to_owned(),
                    ),
                    Err(error) => AdapterEvent::OperationFailed(error.to_string()),
                }))
            }
            Effect::SaveDraft {
                chat,
                thread_root,
                saved_peer,
                text,
                reply_to,
            } => {
                self.save_draft(chat, thread_root, saved_peer, text, reply_to)
                    .await?;
                Ok(None)
            }
            effect @ Effect::SendMessage { .. } => {
                self.execute_message_send(effect, random_id).await
            }
            Effect::SendPoll {
                chat,
                question,
                options,
                reply_to,
                thread_root,
                saved_peer,
                local_id,
            } => {
                self.persist_poll(PollPersistence {
                    chat,
                    local_id,
                    question: &question,
                    options: &options,
                    reply_to,
                    thread_root,
                    saved_peer,
                    delivery: DeliveryState::Pending,
                })
                .await?;
                let result = self
                    .client
                    .send_poll(intuigram_telegram::PollSend {
                        chat,
                        question: question.clone(),
                        options: options.clone(),
                        reply_to,
                        thread_root,
                        monoforum_peer: saved_peer,
                        random_id: random_id.expect("every queued poll has an idempotency token"),
                    })
                    .await;
                let result = match result {
                    Err(source) if source.is_connection_failure() => {
                        return Err(Error::Telegram { source });
                    }
                    result => result,
                };
                match &result {
                    Ok(server_id) => {
                        self.acknowledge_poll(
                            PollPersistence {
                                chat,
                                local_id,
                                question: &question,
                                options: &options,
                                reply_to,
                                thread_root,
                                saved_peer,
                                delivery: DeliveryState::Sent,
                            },
                            *server_id,
                        )
                        .await?;
                    }
                    Err(_) => {
                        self.persist_poll(PollPersistence {
                            chat,
                            local_id,
                            question: &question,
                            options: &options,
                            reply_to,
                            thread_root,
                            saved_peer,
                            delivery: DeliveryState::Failed,
                        })
                        .await?;
                    }
                }
                Ok(Some(match result {
                    Ok(server_id) => AdapterEvent::RichMediaAcknowledged {
                        chat,
                        local_id,
                        server_id,
                    },
                    Err(error) => AdapterEvent::PollFailed {
                        chat,
                        local_id,
                        thread_root,
                        saved_peer,
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
                attachments,
                draft_attachments,
            } => {
                self.edit_message(
                    chat,
                    *message,
                    draft_text,
                    attachments,
                    draft_attachments,
                    random_id.expect("every queued media edit has an idempotency token"),
                )
                .await
            }
            Effect::DeleteMessages { chat, messages } => self.delete_messages(chat, messages).await,
            Effect::ForwardMessages {
                source,
                destination,
                destination_saved_peer,
                messages,
            } => {
                self.forward_messages(
                    source,
                    destination,
                    destination_saved_peer,
                    messages,
                    random_id.expect("every queued forward has an idempotency token"),
                )
                .await
            }
            Effect::ReactMessage {
                chat,
                message,
                reaction,
            } => self.react_message(chat, *message, reaction).await,
            Effect::SetMessagePinned {
                chat: _,
                message: _,
                pinned: _,
            } => MisroutedPinEffectSnafu.fail(),
            Effect::VotePoll {
                chat,
                message,
                options,
            } => self.vote_poll(chat, *message, options).await,
            effect @ (Effect::RefreshSpecialized { .. }
            | Effect::ToggleTodoItem { .. }
            | Effect::AppendTodoItem { .. }) => self.execute_specialized(effect).await,
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
            Effect::LoadAvatar { avatar } => Ok(Some(match self.load_avatar(avatar).await {
                Ok(Some(image)) => AdapterEvent::AvatarReady(AvatarView { avatar, image }),
                Ok(None) => AdapterEvent::AvatarFailed { avatar },
                Err(Error::Telegram { source }) if source.is_connection_failure() => {
                    return Err(Error::Telegram { source });
                }
                Err(_) => AdapterEvent::AvatarFailed { avatar },
            })),
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
