pub(super) fn render_folders(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    let content_area = mode.horizontally_padded(area);
    let mut x = content_area.x;
    for (index, folder) in view.folders.iter().enumerate() {
        let width = u16::try_from(folder.title.chars().count().saturating_add(3))
            .unwrap_or(u16::MAX)
            .min(content_area.right().saturating_sub(x));
        semantics.push(SemanticNode {
            role: SemanticRole::Folder,
            name: folder.title.clone(),
            description: None,
            domain_id: Some(i64::from(folder.id)),
            action: None,
            delivery: None,
            active: index == view.active_folder,
            focused: view.focus == Focus::Chats,
            bounds: Rect::new(x, content_area.y, width, content_area.height),
        });
        x = x.saturating_add(width);
    }
    let leading = if mode == ViewMode::Default { "" } else { "  " };
    let trailing = if mode == ViewMode::Default { "  " } else { " " };
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
            Span::raw(leading),
            Span::styled(format!("{}{unread}", folder.title), style),
            Span::raw(trailing),
        ]
    });
    let folders = Line::from(spans.collect::<Vec<_>>());
    let lines = match mode {
        ViewMode::Default => vec![Line::from(""), folders, Line::from("")],
        ViewMode::Compact => vec![folders],
    };
    frame.render_widget(Paragraph::new("").style(surface_style(false)), area);
    frame.render_widget(
        Paragraph::new(lines).style(surface_style(false)),
        content_area,
    );
}

pub(super) fn render_actions(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    keymap: &EffectiveKeymap,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    let content_area = mode.horizontally_padded(area);
    let mut spans = Vec::new();
    let mut x = content_area.x;
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
        .min(content_area.right().saturating_sub(x));
        semantics.push(SemanticNode {
            role: SemanticRole::Action,
            name: binding.label.to_owned(),
            description: Some(binding.key.label()),
            domain_id: None,
            action: Some(binding.action),
            delivery: None,
            active: true,
            focused: false,
            bounds: Rect::new(x, content_area.y, width, content_area.height),
        });
        x = x.saturating_add(width);
        spans.push(Span::styled(
            binding.key.label(),
            Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(format!(" {}  ", binding.label)));
    }
    frame.render_widget(Paragraph::new("").style(surface_style(false)), area);
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(surface_style(false)),
        content_area,
    );
}

pub(super) fn render_status(frame: &mut Frame<'_>, area: Rect, view: &View, mode: ViewMode) {
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
    frame.render_widget(Paragraph::new("").style(style), area);
    frame.render_widget(
        Paragraph::new(status).style(style),
        mode.horizontally_padded(area),
    );
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
