use super::*;
use crate::source::render_text::capped_text;

pub(super) fn render_saved_dialogs(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
    graphics: &mut GraphicsFrame,
) {
    let focused = view.focus == Focus::SavedDialogs;
    let direct_messages = view
        .active_chat
        .and_then(|index| view.chats.get(index))
        .is_some_and(|chat| chat.has_direct_messages);
    let list_name = if direct_messages {
        "Direct Messages"
    } else {
        "Saved Messages"
    };
    frame.render_widget(Paragraph::new("").style(surface_style(focused)), area);
    let area = mode.horizontally_padded(area);
    semantics.push(SemanticNode {
        role: SemanticRole::SavedDialogList,
        name: list_name.to_owned(),
        description: None,
        domain_id: view
            .active_chat
            .and_then(|index| view.chats.get(index))
            .map(|chat| chat.id.0),
        action: None,
        delivery: None,
        active: false,
        selected: false,
        focused,
        bounds: area,
    });
    if view.saved_dialogs.is_empty() {
        let text = if view.saved_dialogs_loading {
            Line::from(effort_spans(
                if direct_messages {
                    "⌁ intuigram · connecting conversations"
                } else {
                    "⌁ intuigram · sorting Saved Messages"
                },
                view.animation_frame,
            ))
        } else {
            Line::from(Span::styled(
                if direct_messages {
                    "No direct messages"
                } else {
                    "No saved dialogs"
                },
                Style::default().fg(MUTED_TEXT),
            ))
        };
        frame.render_widget(
            Paragraph::new(text)
                .alignment(Alignment::Center)
                .style(surface_style(focused)),
            area,
        );
        return;
    }
    let item_height = mode.item_height(2);
    let visible = (usize::from(area.height) / usize::from(item_height)).max(1);
    let active = view
        .active_saved_dialog
        .unwrap_or(0)
        .min(view.saved_dialogs.len() - 1);
    let start = active
        .saturating_sub(visible / 2)
        .min(view.saved_dialogs.len().saturating_sub(visible));
    let end = (start + visible).min(view.saved_dialogs.len());
    semantics.extend(
        view.saved_dialogs[start..end]
            .iter()
            .enumerate()
            .map(|(offset, dialog)| {
                let index = start + offset;
                SemanticNode {
                    role: SemanticRole::SavedDialog,
                    name: dialog.title.clone(),
                    description: Some(dialog.preview.clone()),
                    domain_id: Some(dialog.peer.0),
                    action: None,
                    delivery: None,
                    active: view.active_saved_dialog == Some(index),
                    selected: false,
                    focused,
                    bounds: Rect::new(
                        area.x,
                        area.y
                            .saturating_add((offset as u16).saturating_mul(item_height)),
                        area.width,
                        item_height,
                    ),
                }
            }),
    );
    let items = view.saved_dialogs[start..end]
        .iter()
        .enumerate()
        .map(|(offset, dialog)| {
            let selected = view.active_saved_dialog == Some(start + offset);
            let marker = if dialog.pinned { " ●" } else { "" };
            let unread = if dialog.unread > 0 {
                format!(" {}", dialog.unread)
            } else if dialog.unread_mark {
                " unread".to_owned()
            } else {
                String::new()
            };
            let metadata = format!("{}{}{}", dialog.timestamp, marker, unread);
            let rule_width = Line::from(selection_rule(selected)).width();
            let avatar_width = avatar_width(view, Some(dialog.peer), &dialog.title, graphics);
            let title_width = usize::from(area.width)
                .saturating_sub(rule_width)
                .saturating_sub(avatar_width)
                .saturating_sub(Line::from(metadata.as_str()).width())
                .saturating_sub(1);
            let title = capped_text(&dialog.title, title_width);
            let gap = usize::from(area.width)
                .saturating_sub(rule_width)
                .saturating_sub(avatar_width)
                .saturating_sub(Line::from(title.as_str()).width())
                .saturating_sub(Line::from(metadata.as_str()).width());
            let mut title_line = vec![selection_rule(selected)];
            title_line.extend(avatar_spans(
                view,
                Some(dialog.peer),
                &dialog.title,
                Some(avatar_image_id(dialog.peer, 0x5341_5645)),
                graphics,
                focused,
            ));
            title_line.extend([
                Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" ".repeat(gap)),
                Span::styled(metadata, Style::default().fg(MUTED_TEXT)),
            ]);
            let mut lines = vec![Line::from(title_line)];
            let preview = dialog.draft.as_ref().map_or_else(
                || dialog.preview.replace('\n', " "),
                |draft| format!("Draft · {}", draft.text.replace('\n', " ")),
            );
            lines.push(Line::from(vec![
                selection_rule(selected),
                Span::raw(" ".repeat(avatar_width)),
                Span::styled(
                    capped_text(
                        &preview,
                        usize::from(area.width)
                            .saturating_sub(rule_width)
                            .saturating_sub(avatar_width),
                    ),
                    Style::default().fg(MUTED_TEXT),
                ),
            ]));
            if mode == ViewMode::Default {
                lines.push(Line::from(""));
            }
            ListItem::new(lines)
        });
    frame.render_widget(List::new(items).style(surface_style(focused)), area);
}
