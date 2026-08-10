#[cfg(test)]
pub(crate) fn render(frame: &mut Frame<'_>, view: &View, keymap: &EffectiveKeymap) {
    render_with_options(frame, view, keymap, ViewOptions::default());
}

#[cfg(test)]
fn render_with_options(
    frame: &mut Frame<'_>,
    view: &View,
    keymap: &EffectiveKeymap,
    options: ViewOptions,
) {
    render_with_semantics(frame, view, keymap, options, &mut Vec::new());
}

#[cfg(test)]
fn render_with_semantics(
    frame: &mut Frame<'_>,
    view: &View,
    keymap: &EffectiveKeymap,
    options: ViewOptions,
    semantics: &mut Vec<SemanticNode>,
) {
    let mut chat_viewport = ChatViewport::default();
    render_with_graphics(
        frame,
        view,
        keymap,
        options,
        semantics,
        &mut GraphicsFrame::new(GraphicsProtocol::Text, rasterm::Multiplexer::None),
        &mut chat_viewport,
    );
}

pub(super) fn render_with_graphics(
    frame: &mut Frame<'_>,
    view: &View,
    keymap: &EffectiveKeymap,
    options: ViewOptions,
    semantics: &mut Vec<SemanticNode>,
    graphics: &mut GraphicsFrame,
    chat_viewport: &mut ChatViewport,
) {
    let mode = options.mode;
    let area = frame.area();
    frame.render_widget(Clear, area);
    let rows = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(mode.folder_height()),
        Constraint::Length(mode.chrome_row_height()),
    ])
    .split(area);
    render_main(
        frame,
        rows[0],
        view,
        options,
        semantics,
        graphics,
        chat_viewport,
    );
    render_folders(frame, rows[1], view, mode, semantics);
    render_bottom_chrome(frame, rows[2], view, keymap, mode, semantics);
    if view.folder_manager.is_some() {
        render_folder_manager(frame, area, view);
    } else if view.scheduled.is_some() {
        render_scheduled(frame, area, view);
    } else if view.rich_media.is_some() {
        render_rich_media(frame, area, view);
    } else if view.action_menu.is_some() {
        render_action_menu(frame, area, view);
    } else if view.account_confirmation.is_some() {
        render_account_confirmation(frame, area, view);
    } else if view.account_picker.is_some() {
        render_account_picker(frame, area, view);
    } else if view.help_open {
        render_help(frame, area, view, keymap);
    } else if view.attachment_path.is_some() {
        render_attachment_path(frame, area, view);
    } else if view.save_as.is_some() {
        render_save_as(frame, area, view);
    } else if view.link_confirmation.is_some() {
        render_link_confirmation(frame, area, view);
    } else if view.reaction_picker.is_some() {
        render_reaction_picker(frame, area, view);
    } else if view.poll_vote.is_some() {
        render_poll_vote(frame, area, view);
    } else if view.todo_editor.is_some() {
        render_todo_editor(frame, area, view);
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
    options: ViewOptions,
    semantics: &mut Vec<SemanticNode>,
    graphics: &mut GraphicsFrame,
    chat_viewport: &mut ChatViewport,
) {
    let mode = options.mode;
    if area.width < 80 {
        let chat_list_level = view.focus == Focus::Chats
            || view
                .search
                .as_ref()
                .is_some_and(|search| search.scope == SearchScope::Account);
        if chat_list_level {
            render_chats(frame, area, view, mode, semantics, graphics, chat_viewport);
        } else {
            render_active_chat(frame, area, view, options, semantics, graphics);
        }
        return;
    }
    if render_thread_details(
        frame,
        area,
        view,
        options,
        semantics,
        graphics,
        chat_viewport,
    ) {
        return;
    }
    let columns = if area.width >= 140 {
        Layout::horizontal([
            Constraint::Length(40),
            Constraint::Length(1),
            Constraint::Min(48),
        ])
        .split(area)
    } else {
        Layout::horizontal([
            Constraint::Length(32),
            Constraint::Length(1),
            Constraint::Min(40),
        ])
        .split(area)
    };
    render_chats(
        frame,
        columns[0],
        view,
        mode,
        semantics,
        graphics,
        chat_viewport,
    );
    render_active_chat(frame, columns[2], view, options, semantics, graphics);
}

pub(super) fn render_chats(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
    graphics: &mut GraphicsFrame,
    chat_viewport: &mut ChatViewport,
) {
    let focused = view.focus == Focus::Chats;
    let rows = Layout::vertical([
        Constraint::Length(mode.chat_header_height()),
        Constraint::Min(1),
    ])
    .split(area);
    render_chat_list_header(frame, rows[0], view, mode, focused);
    frame.render_widget(Paragraph::new("").style(surface_style(focused)), rows[1]);
    let list_area = mode.chat_list_area(rows[1]);
    let items_area = match mode {
        ViewMode::Default if list_area.width > 1 => Rect::new(
            list_area.x,
            list_area.y,
            list_area.width.saturating_sub(1),
            list_area.height,
        ),
        ViewMode::Default | ViewMode::Compact => list_area,
    };
    semantics.push(SemanticNode {
        role: SemanticRole::ChatList,
        name: "Chats".to_owned(),
        description: None,
        domain_id: None,
        action: None,
        delivery: None,
        active: false,
        selected: false,
        focused,
        bounds: list_area,
    });
    let item_height = mode.item_height(2);
    let visible_items = usize::from(list_area.height) / usize::from(item_height);
    let range = chat_viewport.window(
        view.chats.len(),
        view.active_chat,
        visible_items,
        false,
        view.chat_scroll_direction,
    );
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
                selected: false,
                focused,
                bounds: Rect::new(
                    items_area.x,
                    items_area
                        .y
                        .saturating_add((offset as u16).saturating_mul(item_height)),
                    items_area.width,
                    item_height.min(
                        items_area
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
            let rule = selection_rule(selected);
            let [avatar_top, avatar_bottom] = avatar_block(
                view,
                Some(chat.id),
                &chat.title,
                Some(avatar_image_id(chat.id, chat.id.0 ^ 0x4348_4154)),
                graphics,
                focused,
            );
            let marker = Span::styled(marker, Style::default().fg(MUTED_TEXT));
            let unread = Span::styled(unread, Style::default().fg(PRIMARY));
            let timestamp = if chat.preview_timestamp.is_empty() {
                String::new()
            } else {
                format!(" {}", chat.preview_timestamp)
            };
            let timestamp = Span::styled(timestamp, Style::default().fg(MUTED_TEXT));
            let fixed_width = Line::from(
                [rule.clone()]
                    .into_iter()
                    .chain(avatar_top.clone())
                    .chain([timestamp.clone(), marker.clone(), unread.clone()])
                    .collect::<Vec<_>>(),
            )
            .width();
            let title = capped_text(
                &chat.title,
                usize::from(items_area.width).saturating_sub(fixed_width),
            );
            let title_width = Line::from(title.as_str()).width();
            let title_gap = usize::from(items_area.width)
                .saturating_sub(fixed_width)
                .saturating_sub(title_width);
            let preview = chat
                .preview_sender
                .as_deref()
                .filter(|_| {
                    matches!(
                        chat.kind,
                        ChatKind::BasicGroup | ChatKind::Supergroup | ChatKind::Gigagroup
                    )
                })
                .map_or_else(
                    || chat.preview.clone(),
                    |sender| format!("{sender}: {}", chat.preview),
                );
            let preview_width = usize::from(items_area.width)
                .saturating_sub(Line::from(selection_rule(selected)).width())
                .saturating_sub(Line::from(avatar_bottom.clone()).width());
            let mut title_line = vec![rule];
            title_line.extend(avatar_top);
            title_line.extend([
                Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" ".repeat(title_gap)),
                timestamp,
                marker,
                unread,
            ]);
            let mut preview_line = vec![selection_rule(selected)];
            preview_line.extend(avatar_bottom);
            preview_line.push(Span::styled(
                capped_text(&preview, preview_width),
                Style::default().fg(MUTED_TEXT),
            ));
            let mut lines = vec![Line::from(title_line), Line::from(preview_line)];
            if mode == ViewMode::Default {
                lines.push(Line::from(""));
            }
            ListItem::new(lines)
        });
    frame.render_widget(List::new(items).style(surface_style(focused)), items_area);
}

pub(super) fn render_active_chat(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    options: ViewOptions,
    semantics: &mut Vec<SemanticNode>,
    graphics: &mut GraphicsFrame,
) {
    let mode = options.mode;
    if matches!(view.focus, Focus::Topics | Focus::SavedDialogs) {
        let rows = Layout::vertical([
            Constraint::Length(mode.active_chat_header_height()),
            Constraint::Min(1),
        ])
        .split(area);
        render_active_chat_header(frame, rows[0], view, mode, true, graphics);
        if view.focus == Focus::Topics {
            render_topics(frame, rows[1], view, mode, semantics);
        } else {
            render_saved_dialogs(frame, rows[1], view, mode, semantics, graphics);
        }
        return;
    }
    let composer_height = composer_height(area, view);
    let rows = Layout::vertical([
        Constraint::Length(mode.active_chat_header_height()),
        Constraint::Min(1),
        Constraint::Length(composer_height),
    ])
    .split(area);
    render_active_chat_header(
        frame,
        rows[0],
        view,
        mode,
        matches!(view.focus, Focus::Transcript | Focus::Composer),
        graphics,
    );
    render_transcript(
        frame,
        rows[1],
        view,
        view.focus == Focus::Transcript,
        options,
        semantics,
        graphics,
    );
    if composer_height > 0 {
        render_composer(frame, rows[2], view, semantics);
    }
}
use super::*;
