#[cfg(test)]
pub(crate) fn render(frame: &mut Frame<'_>, view: &View, keymap: &EffectiveKeymap) {
    render_with_mode(frame, view, keymap, ViewMode::Default);
}

pub(crate) fn render_with_mode(
    frame: &mut Frame<'_>,
    view: &View,
    keymap: &EffectiveKeymap,
    mode: ViewMode,
) {
    render_with_semantics(frame, view, keymap, mode, &mut Vec::new());
}

pub(super) fn render_with_semantics(
    frame: &mut Frame<'_>,
    view: &View,
    keymap: &EffectiveKeymap,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    let area = frame.area();
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new("").style(Style::default().fg(TEXT).bg(BACKGROUND)),
        area,
    );
    let rows = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(mode.folder_height()),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    render_main(frame, rows[0], view, mode, semantics);
    render_folders(frame, rows[1], view, mode, semantics);
    render_actions(frame, rows[2], view, keymap, semantics);
    render_status(frame, rows[3], view);
    if view.help_open {
        render_help(frame, area, view, keymap);
    } else if view.link_confirmation.is_some() {
        render_link_confirmation(frame, area, view);
    } else if view.reaction_picker.is_some() {
        render_reaction_picker(frame, area, view);
    } else if view.poll_vote.is_some() {
        render_poll_vote(frame, area, view);
    } else if view.forward_picker.is_some() {
        render_forward_picker(frame, area, view);
    } else if view.delete_confirmation.is_some() {
        render_delete_confirmation(frame, area, view);
    } else if view.folder_picker.is_some() {
        render_folder_picker(frame, area, view);
    }
}

pub(super) fn render_main(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    let columns = if area.width >= 120 {
        Layout::horizontal([
            Constraint::Length(32),
            Constraint::Length(1),
            Constraint::Min(48),
        ])
        .split(area)
    } else {
        Layout::horizontal([
            Constraint::Length(30),
            Constraint::Length(1),
            Constraint::Min(40),
        ])
        .split(area)
    };
    render_chats(frame, columns[0], view, mode, semantics);
    render_active_chat(frame, columns[2], view, mode, semantics);
}

pub(super) fn render_chats(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    let focused = view.focus == Focus::Chats;
    let rows = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).split(area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Chats", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("  {}", view.account_name),
                Style::default().fg(MUTED_TEXT),
            ),
        ]))
        .style(surface_style(focused)),
        rows[0],
    );
    let item_height = mode.item_height(2);
    let visible_items = usize::from(rows[1].height) / usize::from(item_height);
    let range = anchored_window(view.chats.len(), view.active_chat, visible_items, false);
    semantics.extend(
        view.chats[range.clone()]
            .iter()
            .enumerate()
            .map(|(offset, chat)| SemanticNode {
                role: SemanticRole::Chat,
                name: chat.title.clone(),
                description: Some(chat.preview.clone()),
                domain_id: Some(chat.id.0),
                action: None,
                delivery: None,
                active: view.active_chat == Some(range.start + offset),
                focused,
                bounds: Rect::new(
                    rows[1].x,
                    rows[1]
                        .y
                        .saturating_add((offset as u16).saturating_mul(item_height)),
                    rows[1].width,
                    item_height.min(
                        rows[1]
                            .height
                            .saturating_sub((offset as u16).saturating_mul(item_height)),
                    ),
                ),
            }),
    );
    let items = view.chats[range.clone()]
        .iter()
        .enumerate()
        .map(|(offset, chat)| {
            let index = range.start + offset;
            let marker = if chat.pinned { " ●" } else { "" };
            let unread = if chat.unread > 0 {
                format!(" {}", chat.unread)
            } else {
                String::new()
            };
            let selected = view.active_chat == Some(index);
            let mut lines = vec![
                Line::from(vec![
                    selection_rule(selected),
                    Span::styled(
                        chat.title.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(marker, Style::default().fg(MUTED_TEXT)),
                    Span::styled(unread, Style::default().fg(PRIMARY)),
                ]),
                Line::from(vec![
                    selection_rule(selected),
                    Span::styled(chat.preview.clone(), Style::default().fg(MUTED_TEXT)),
                ]),
            ];
            if mode == ViewMode::Default {
                lines.push(Line::from(""));
            }
            ListItem::new(lines)
        });
    frame.render_widget(List::new(items).style(surface_style(focused)), rows[1]);
}

pub(super) fn render_active_chat(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    let composer_height = composer_height(area, view);
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(composer_height),
    ])
    .split(area);
    let header = view
        .active_chat
        .and_then(|index| view.chats.get(index))
        .map_or_else(
            || {
                Line::from(Span::styled(
                    "No active Chat",
                    Style::default().fg(MUTED_TEXT),
                ))
            },
            |chat| {
                Line::from(vec![
                    Span::styled(
                        chat.title.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        if let Some(root) = view.active_thread {
                            format!("  Thread {}", root.0)
                        } else if chat.unread > 0 {
                            format!("  {} unread", chat.unread)
                        } else {
                            "  up to date".to_owned()
                        },
                        Style::default().fg(MUTED_TEXT),
                    ),
                ])
            },
        );
    let active_message = view
        .active_message
        .and_then(|index| view.messages.get(index))
        .map_or_else(
            || Line::from(""),
            |message| {
                Line::from(vec![
                    selection_rule(true),
                    Span::styled(
                        "Active message",
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" · {} · {}", message.sender, message.timestamp),
                        Style::default().fg(MUTED_TEXT),
                    ),
                ])
            },
        );
    let pinned = view
        .pinned_messages
        .iter()
        .rev()
        .find(|message| message.details.pinned)
        .map(|message| {
            Span::styled(
                format!("Pinned · {}", message.body.replace('\n', " ")),
                Style::default().fg(MUTED_TEXT),
            )
        });
    let subheader = if let Some(pinned) = pinned {
        let mut spans = vec![pinned];
        if !active_message.spans.is_empty() {
            spans.push(Span::raw("  "));
            spans.extend(active_message.spans);
        }
        Line::from(spans)
    } else {
        active_message
    };
    frame.render_widget(
        Paragraph::new(vec![header, subheader])
            .style(surface_style(view.focus == Focus::Transcript)),
        rows[0],
    );
    render_transcript(
        frame,
        rows[1],
        view,
        view.focus == Focus::Transcript,
        mode,
        semantics,
    );
    if composer_height > 0 {
        render_composer(frame, rows[2], view, semantics);
    }
}
use super::*;
