pub(super) fn render_composer(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    semantics: &mut Vec<SemanticNode>,
) {
    let focused = view.focus == Focus::Composer;
    semantics.push(SemanticNode {
        role: SemanticRole::Composer,
        name: "Composer".to_owned(),
        description: Some(view.composer.text.clone()),
        domain_id: None,
        action: None,
        delivery: None,
        active: true,
        focused,
        bounds: area,
    });
    let composer_label = if view.poll_composer {
        "Poll · question first, then one option per line".to_owned()
    } else {
        view.composer.editing.map_or_else(
            || {
                view.composer.reply_to.map_or_else(
                    || "Draft".to_owned(),
                    |message| format!("Reply to {}", message.0),
                )
            },
            |message| format!("Edit Message {}", message.0),
        )
    };
    let composer_label = if view.composer.attachments.is_empty() {
        composer_label
    } else {
        format!(
            "{composer_label} · {} attachment(s)",
            view.composer.attachments.len()
        )
    };
    let composer_label_width = u16::try_from(composer_label.chars().count()).unwrap_or(u16::MAX);
    let draft_content = if view.composer.text.is_empty() {
        Span::styled("Type or paste a message…", Style::default().fg(MUTED_TEXT))
    } else {
        Span::raw(view.composer.text.clone())
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::raw(" "),
                interaction_rule(focused),
                Span::styled(composer_label, Style::default().fg(MUTED_TEXT)),
                Span::raw("  "),
                draft_content,
            ]),
            Line::from(""),
        ])
        .style(surface_style(focused))
        .wrap(Wrap { trim: false }),
        area,
    );
    if focused && !overlay_open(view) {
        frame.set_cursor_position(composer_cursor(
            area,
            &view.composer.text,
            composer_label_width,
        ));
    }
}

fn composer_cursor(area: Rect, text: &str, context_width: u16) -> (u16, u16) {
    let content_x = area
        .x
        .saturating_add(5)
        .saturating_add(context_width)
        .min(area.right().saturating_sub(1));
    let content_width = area.right().saturating_sub(content_x).max(1);
    let explicit_lines = text.matches('\n').count() as u16;
    let last_line_width = Line::from(text.rsplit('\n').next().unwrap_or_default()).width();
    let last_line_width = u16::try_from(last_line_width).unwrap_or(u16::MAX);
    let wrapped_lines = last_line_width / content_width;
    let x = content_x
        .saturating_add(last_line_width % content_width)
        .min(area.right().saturating_sub(1));
    let y = area
        .y
        .saturating_add(1)
        .saturating_add(explicit_lines)
        .saturating_add(wrapped_lines)
        .min(area.bottom().saturating_sub(1));
    (x, y)
}

fn overlay_open(view: &View) -> bool {
    view.help_open
        || view.link_confirmation.is_some()
        || view.reaction_picker.is_some()
        || view.poll_vote.is_some()
        || view.forward_picker.is_some()
        || view.delete_confirmation.is_some()
        || view.folder_picker.is_some()
}

pub(super) fn render_folders(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    let mut x = area.x;
    for (index, folder) in view.folders.iter().enumerate() {
        let width = u16::try_from(folder.title.chars().count().saturating_add(3))
            .unwrap_or(u16::MAX)
            .min(area.right().saturating_sub(x));
        semantics.push(SemanticNode {
            role: SemanticRole::Folder,
            name: folder.title.clone(),
            description: None,
            domain_id: Some(i64::from(folder.id)),
            action: None,
            delivery: None,
            active: index == view.active_folder,
            focused: view.focus == Focus::Chats,
            bounds: Rect::new(x, area.y, width, area.height),
        });
        x = x.saturating_add(width);
    }
    let spans = view.folders.iter().enumerate().flat_map(|(index, folder)| {
        let active = index == view.active_folder;
        let style = if active {
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(MUTED_TEXT)
        };
        let unread = if folder.unread > 0 {
            format!(" {}", folder.unread)
        } else {
            String::new()
        };
        [
            Span::raw("  "),
            Span::styled(format!("{}{unread}", folder.title), style),
            Span::raw(" "),
        ]
    });
    let folders = Line::from(spans.collect::<Vec<_>>());
    let lines = match mode {
        ViewMode::Default => vec![Line::from(""), folders, Line::from("")],
        ViewMode::Compact => vec![folders],
    };
    frame.render_widget(Paragraph::new(lines).style(surface_style(false)), area);
}

pub(super) fn render_actions(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    keymap: &EffectiveKeymap,
    semantics: &mut Vec<SemanticNode>,
) {
    let mut spans = Vec::new();
    let mut x = area.x;
    for binding in keymap.action_bar(view) {
        let width = u16::try_from(
            binding
                .key
                .label()
                .chars()
                .count()
                .saturating_add(binding.label.chars().count())
                .saturating_add(3),
        )
        .unwrap_or(u16::MAX)
        .min(area.right().saturating_sub(x));
        semantics.push(SemanticNode {
            role: SemanticRole::Action,
            name: binding.label.to_owned(),
            description: Some(binding.key.label()),
            domain_id: None,
            action: Some(binding.action),
            delivery: None,
            active: true,
            focused: false,
            bounds: Rect::new(x, area.y, width, area.height),
        });
        x = x.saturating_add(width);
        spans.push(Span::styled(
            binding.key.label(),
            Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(format!(" {}  ", binding.label)));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(surface_style(false)),
        area,
    );
}

pub(super) fn render_status(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let connection = match view.connection {
        ConnectionState::Connected => "connected",
        ConnectionState::Connecting => "connecting",
        ConnectionState::ReconnectCooldown => "reconnect cooldown",
    };
    let mut status = format!("{} · {:?} · {connection}", view.account_name, view.focus);
    if let Some(search) = &view.search {
        write!(status, " · {:?} search: {}", search.scope, search.query)
            .expect("writing to a String cannot fail");
    }
    if view.has_newer_messages {
        status.push_str(" · new messages ↓");
    }
    if let Some(notice) = &view.notice {
        write!(status, " · {notice}").expect("writing to a String cannot fail");
    }
    let style = if view.focus == Focus::Search {
        surface_style(true)
    } else {
        Style::default().fg(MUTED_TEXT).bg(CHROME_BACKGROUND)
    };
    frame.render_widget(Paragraph::new(status).style(style), area);
}

pub(super) fn render_help(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    keymap: &EffectiveKeymap,
) {
    let popup = centered_rect(70, 75, area);
    let lines = std::iter::once(Line::from(Span::styled(
        "Context Help",
        Style::default().add_modifier(Modifier::BOLD),
    )))
    .chain(std::iter::once(Line::from("")))
    .chain(keymap.help(view).map(|binding| {
        Line::from(vec![
            Span::styled(
                format!("{:>12}", binding.key.label()),
                Style::default().fg(PRIMARY),
            ),
            Span::raw(format!("  {}", binding.label)),
        ])
    }));
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines.collect::<Vec<_>>())
            .style(surface_style(true))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

pub(super) fn render_folder_picker(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let popup = centered_rect(52, 60, area);
    let memberships = view
        .active_chat
        .and_then(|index| view.chats.get(index))
        .map(|chat| chat.folders.as_slice())
        .unwrap_or_default();
    let lines = std::iter::once(Line::from(Span::styled(
        "Folder membership",
        Style::default().add_modifier(Modifier::BOLD),
    )))
    .chain(std::iter::once(Line::from(Span::styled(
        "Choose a Folder to add or remove this Chat",
        Style::default().fg(MUTED_TEXT),
    ))))
    .chain(std::iter::once(Line::from("")))
    .chain(
        view.folders
            .iter()
            .enumerate()
            .skip(1)
            .map(|(index, folder)| {
                let selected = view.folder_picker == Some(index);
                let marker = if memberships.contains(&folder.id) {
                    "[x]"
                } else {
                    "[ ]"
                };
                Line::from(vec![
                    selection_rule(selected),
                    Span::styled(
                        format!("{marker} {}", folder.title),
                        Style::default().fg(if selected { PRIMARY } else { TEXT }),
                    ),
                ])
            }),
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines.collect::<Vec<_>>())
            .style(surface_style(true))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

pub(super) fn selection_rule(selected: bool) -> Span<'static> {
    if selected {
        Span::styled("│ ", Style::default().fg(PRIMARY))
    } else {
        Span::raw("  ")
    }
}

pub(super) fn interaction_rule(active: bool) -> Span<'static> {
    if active {
        Span::styled("│ ", Style::default().fg(PRIMARY))
    } else {
        Span::styled("│ ", Style::default().fg(MUTED_TEXT))
    }
}

pub(super) fn surface_style(focused: bool) -> Style {
    Style::default().fg(TEXT).bg(if focused {
        FOCUSED_SURFACE_BACKGROUND
    } else {
        SURFACE_BACKGROUND
    })
}

pub(super) fn anchored_window(
    length: usize,
    active: Option<usize>,
    visible_items: usize,
    default_to_end: bool,
) -> std::ops::Range<usize> {
    let visible_items = visible_items.max(1).min(length);
    let active = active
        .map(|index| index.min(length.saturating_sub(1)))
        .or_else(|| default_to_end.then(|| length.saturating_sub(1)))
        .unwrap_or(0);
    let anchor = visible_items / 3;
    let start = active
        .saturating_sub(anchor)
        .min(length.saturating_sub(visible_items));
    start..start + visible_items
}

pub(super) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}
use super::*;
