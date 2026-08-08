#[cfg(test)]
pub(crate) fn render(frame: &mut Frame<'_>, view: &View, keymap: &EffectiveKeymap) {
    render_with_mode(frame, view, keymap, ViewMode::Default);
}

#[cfg(test)]
pub(crate) fn render_with_mode(
    frame: &mut Frame<'_>,
    view: &View,
    keymap: &EffectiveKeymap,
    mode: ViewMode,
) {
    render_with_semantics(frame, view, keymap, mode, &mut Vec::new());
}

#[cfg(test)]
fn render_with_semantics(
    frame: &mut Frame<'_>,
    view: &View,
    keymap: &EffectiveKeymap,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    render_with_graphics(
        frame,
        view,
        keymap,
        mode,
        semantics,
        &mut GraphicsFrame::new(GraphicsProtocol::Text),
    );
}

pub(super) fn render_with_graphics(
    frame: &mut Frame<'_>,
    view: &View,
    keymap: &EffectiveKeymap,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
    graphics: &mut GraphicsFrame,
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
        Constraint::Length(mode.chrome_row_height()),
        Constraint::Length(mode.chrome_row_height()),
    ])
    .split(area);
    render_main(frame, rows[0], view, mode, semantics, graphics);
    render_folders(frame, rows[1], view, mode, semantics);
    render_actions(frame, rows[2], view, keymap, mode, semantics);
    render_status(frame, rows[3], view, mode);
    if view.folder_manager.is_some() {
        render_folder_manager(frame, area, view);
    } else if view.scheduled.is_some() {
        render_scheduled(frame, area, view);
    } else if view.rich_media.is_some() {
        render_rich_media(frame, area, view);
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
    graphics: &mut GraphicsFrame,
) {
    if area.width < 80 {
        let chat_list_level = view.focus == Focus::Chats
            || view
                .search
                .as_ref()
                .is_some_and(|search| search.scope == SearchScope::Account);
        if chat_list_level {
            render_chats(frame, area, view, mode, semantics);
        } else {
            render_active_chat(frame, area, view, mode, semantics, graphics);
        }
        return;
    }
    let columns = if area.width >= 140 {
        Layout::horizontal([
            Constraint::Length(38),
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
    render_active_chat(frame, columns[2], view, mode, semantics, graphics);
}

pub(super) fn render_chats(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    let focused = view.focus == Focus::Chats;
    let rows = Layout::vertical([
        Constraint::Length(mode.chat_header_height()),
        Constraint::Min(1),
    ])
    .split(area);
    render_chat_list_header(frame, rows[0], view, mode, focused);
    frame.render_widget(Paragraph::new("").style(surface_style(focused)), rows[1]);
    let list_area = mode.padded(rows[1]);
    let item_height = mode.item_height(2);
    let visible_items = usize::from(list_area.height) / usize::from(item_height);
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
                selected: false,
                focused,
                bounds: Rect::new(
                    list_area.x,
                    list_area
                        .y
                        .saturating_add((offset as u16).saturating_mul(item_height)),
                    list_area.width,
                    item_height.min(
                        list_area
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
                    avatar_badge(&chat.title),
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
    frame.render_widget(List::new(items).style(surface_style(focused)), list_area);
}

pub(super) fn render_active_chat(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
    graphics: &mut GraphicsFrame,
) {
    let composer_height = composer_height(area, view);
    let rows = Layout::vertical([
        Constraint::Length(mode.active_chat_header_height()),
        Constraint::Min(1),
        Constraint::Length(composer_height),
    ])
    .split(area);
    render_active_chat_header(frame, rows[0], view, mode, view.focus == Focus::Transcript);
    render_transcript(
        frame,
        rows[1],
        view,
        view.focus == Focus::Transcript,
        mode,
        semantics,
        graphics,
    );
    if composer_height > 0 {
        render_composer(frame, rows[2], view, semantics);
    }
}
use super::*;
