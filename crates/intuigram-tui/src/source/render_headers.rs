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
) {
    let (title, status) = title_and_status(view);
    let context = active_context(view);
    let lines = match mode {
        ViewMode::Default => {
            let mut status = status;
            append_context(&mut status, context);
            vec![Line::from(""), title, status, Line::from("")]
        }
        ViewMode::Compact => {
            let mut title = title;
            if !status.spans.is_empty() {
                title.spans.push(Span::raw("  "));
                title.spans.extend(status.spans);
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
                let status = match view.chat_loading {
                    ChatLoadingState::Updating => {
                        Line::from(effort_spans("updating", view.animation_frame))
                    }
                    ChatLoadingState::Fresh => Line::from(""),
                    ChatLoadingState::Idle => {
                        let status = if let Some(root) = view.active_thread {
                            format!("Thread {}", root.0)
                        } else if chat.unread > 0 {
                            format!("{} unread", chat.unread)
                        } else {
                            "up to date".to_owned()
                        };
                        Line::from(Span::styled(status, Style::default().fg(MUTED_TEXT)))
                    }
                };
                (
                    Line::from(Span::styled(
                        chat.title.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    status,
                )
            },
        )
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
