use super::*;

pub(super) fn render_chat_list_header(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    mode: ViewMode,
    focused: bool,
) {
    let content = Line::from(vec![
        Span::styled("Chats", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("  {}", view.account_name),
            Style::default().fg(MUTED_TEXT),
        ),
    ]);
    let lines = match mode {
        ViewMode::Default => vec![Line::from(""), content, Line::from("")],
        ViewMode::Compact => vec![content],
    };
    frame.render_widget(Paragraph::new("").style(surface_style(focused)), area);
    frame.render_widget(
        Paragraph::new(lines).style(surface_style(focused)),
        mode.horizontally_padded(area),
    );
}

pub(super) fn render_active_chat_header(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    mode: ViewMode,
    focused: bool,
    graphics: &mut GraphicsFrame,
) {
    let (mut title, status) = title_and_status(view);
    let mut context = active_context(view);
    let avatar = header_avatar(view, graphics, focused);
    let lines = match mode {
        ViewMode::Default => {
            let mut status = status;
            append_context(&mut status, context);
            if let Some([top, bottom]) = avatar {
                title.spans.splice(0..0, top);
                status.spans.splice(0..0, bottom);
            }
            vec![Line::from(""), title, status, Line::from("")]
        }
        ViewMode::Compact => {
            if !status.spans.is_empty() {
                title.spans.push(Span::raw("  "));
                title.spans.extend(status.spans);
            }
            if let Some([top, bottom]) = avatar {
                title.spans.splice(0..0, top);
                context.spans.splice(0..0, bottom);
            }
            vec![title, context]
        }
    };
    frame.render_widget(Paragraph::new("").style(surface_style(focused)), area);
    frame.render_widget(
        Paragraph::new(lines).style(surface_style(focused)),
        mode.horizontally_padded(area),
    );
}

fn title_and_status(view: &View) -> (Line<'static>, Line<'static>) {
    view.active_chat
        .and_then(|index| view.chats.get(index))
        .map_or_else(
            || {
                (
                    Line::from(Span::styled(
                        "No active Chat",
                        Style::default().fg(MUTED_TEXT),
                    )),
                    Line::from(""),
                )
            },
            |chat| {
                if view.focus == Focus::SavedDialogs {
                    let direct_messages = chat.has_direct_messages;
                    let status = if view.saved_dialogs_loading {
                        Line::from(effort_spans(
                            if direct_messages {
                                "updating Direct Messages"
                            } else {
                                "updating Saved Messages"
                            },
                            view.animation_frame,
                        ))
                    } else {
                        Line::from(Span::styled(
                            if direct_messages {
                                format!("{} direct conversations", view.saved_dialogs.len())
                            } else {
                                format!("{} saved dialogs", view.saved_dialogs.len())
                            },
                            Style::default().fg(MUTED_TEXT),
                        ))
                    };
                    return (title_line(chat), status);
                }
                if let Some(peer) = view.active_saved_peer {
                    let origin = view
                        .saved_dialogs
                        .iter()
                        .find(|dialog| dialog.peer == peer)
                        .map_or("unknown peer", |dialog| dialog.title.as_str());
                    return (
                        title_line(chat),
                        Line::from(Span::styled(
                            if chat.has_direct_messages {
                                format!("Direct message with {origin}")
                            } else {
                                format!("Saved from {origin}")
                            },
                            Style::default().fg(MUTED_TEXT),
                        )),
                    );
                }
                let status = match view.chat_loading {
                    ChatLoadingState::Updating => {
                        Line::from(effort_spans("updating", view.animation_frame))
                    }
                    ChatLoadingState::Fresh => Line::from(""),
                    ChatLoadingState::Idle => {
                        let status = if view.focus == Focus::Topics {
                            if view.topics_loading {
                                return (
                                    title_line(chat),
                                    Line::from(effort_spans(
                                        "updating Topics",
                                        view.animation_frame,
                                    )),
                                );
                            }
                            format!("{} Topics", view.topics.len())
                        } else if let Some(root) = view.active_thread {
                            format!("Thread {}", root.0)
                        } else if !chat.status.is_empty() {
                            chat.status.clone()
                        } else {
                            fallback_status(chat.kind).to_owned()
                        };
                        Line::from(Span::styled(status, Style::default().fg(MUTED_TEXT)))
                    }
                };
                (title_line(chat), status)
            },
        )
}

fn header_avatar(
    view: &View,
    graphics: &mut GraphicsFrame,
    focused: bool,
) -> Option<[Vec<Span<'static>>; 2]> {
    let chat = view.active_chat.and_then(|index| view.chats.get(index))?;
    Some(avatar_block(
        view,
        Some(chat.id),
        &chat.title,
        Some(avatar_image_id(chat.id, 0x4845_4144)),
        graphics,
        focused,
    ))
}

fn title_line(chat: &ChatView) -> Line<'static> {
    Line::from(Span::styled(
        chat.title.clone(),
        Style::default().add_modifier(Modifier::BOLD),
    ))
}

fn fallback_status(kind: ChatKind) -> &'static str {
    match kind {
        ChatKind::SavedMessages => "personal cloud",
        ChatKind::Private => "private chat",
        ChatKind::Bot => "bot",
        ChatKind::BasicGroup | ChatKind::Supergroup | ChatKind::Gigagroup => "group",
        ChatKind::Channel => "channel",
        ChatKind::Inaccessible => "unavailable",
    }
}

fn active_context(view: &View) -> Line<'static> {
    let active_message = view
        .active_message
        .and_then(|index| view.messages.get(index))
        .map_or_else(Vec::new, |message| {
            vec![
                selection_rule(true),
                Span::styled(
                    "Active message",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" · {} · {}", message.sender, message.timestamp),
                    Style::default().fg(MUTED_TEXT),
                ),
            ]
        });
    if let Some(message) = view
        .pinned_messages
        .iter()
        .rev()
        .find(|message| message.details.pinned)
    {
        let mut spans = vec![Span::styled(
            format!("Pinned · {}", message.body.replace('\n', " ")),
            Style::default().fg(MUTED_TEXT),
        )];
        if !active_message.is_empty() {
            spans.push(Span::raw("  "));
            spans.extend(active_message);
        }
        Line::from(spans)
    } else {
        Line::from(active_message)
    }
}

fn append_context(status: &mut Line<'static>, context: Line<'static>) {
    if !context.spans.is_empty() {
        status.spans.push(Span::raw("  "));
        status.spans.extend(context.spans);
    }
}
