pub(in crate::source) fn render_folders(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    let content_area = mode.horizontally_padded(area);
    let leading = if mode == ViewMode::Default { "" } else { "  " };
    let trailing = if mode == ViewMode::Default { "  " } else { " " };
    let mut x = content_area.x;
    for (index, folder) in view.folders.iter().enumerate() {
        let unread = if folder.unread > 0 {
            format!(" {}", folder.unread)
        } else {
            String::new()
        };
        let width = u16::try_from(
            Line::from(format!("{leading}{}{unread}{trailing}", folder.title)).width(),
        )
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
            selected: false,
            focused: view.focus == Focus::Chats,
            bounds: Rect::new(x, content_area.y, width, content_area.height),
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

pub(in crate::source) fn render_bottom_chrome(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    keymap: &EffectiveKeymap,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    let content_area = mode.padded(area);
    let mut spans = status_spans(view, content_area.width);
    let mut x = content_area
        .x
        .saturating_add(u16::try_from(Line::from(spans.clone()).width()).unwrap_or(u16::MAX));
    spans.push(Span::raw("  "));
    x = x.saturating_add(2);
    let mut bindings = keymap.action_bar(view).collect::<Vec<_>>();
    bindings.sort_by_key(|binding| action_bar_rank(binding.key));
    for binding in bindings {
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
            selected: false,
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
    let style = surface_style(view.focus == Focus::Search);
    frame.render_widget(Paragraph::new("").style(style), area);
    frame.render_widget(Paragraph::new(Line::from(spans)).style(style), content_area);
}

const fn action_bar_rank(key: KeyChord) -> u8 {
    match key {
        KeyChord {
            key: Key::Enter,
            control: false,
            shift: false,
            alt: false,
            super_modifier: false,
        } => 0,
        KeyChord {
            key: Key::Char(_),
            control: false,
            shift: false,
            alt: false,
            super_modifier: false,
        } => 1,
        KeyChord {
            key: Key::Up | Key::Down | Key::Left | Key::Right,
            control: false,
            shift: false,
            alt: false,
            super_modifier: false,
        } => 3,
        _ => 2,
    }
}

fn status_spans(view: &View, width: u16) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let sending = view.messages.iter().any(|message| {
        matches!(
            message.delivery,
            DeliveryState::Saving | DeliveryState::Pending
        ) && outbox::message_outbox(view, message).is_none()
    });
    let outbox = outbox::pending_status(view, width);
    let synchronizing = view.chat_loading != ChatLoadingState::Idle;
    let idle = view.connection == ConnectionState::Connected
        && !sending
        && outbox.is_none()
        && !synchronizing;

    match view.connection {
        ConnectionState::Connected if idle => spans.push(Span::raw("connected")),
        ConnectionState::Connected => {}
        ConnectionState::Connecting => {
            spans.extend(effort_spans("connecting", view.animation_frame));
        }
        ConnectionState::ReconnectCooldown => spans.push(Span::raw("reconnect cooldown")),
    }
    if sending {
        append_separator(&mut spans);
        spans.extend(effort_spans(
            "sending",
            view.animation_frame.wrapping_add(3),
        ));
    }
    if let Some(outbox) = outbox {
        append_separator(&mut spans);
        spans.extend(outbox);
    }
    if synchronizing {
        append_separator(&mut spans);
        spans.extend(effort_spans(
            "synchronizing",
            view.animation_frame.wrapping_add(6),
        ));
    }
    if let Some(search) = &view.search {
        append_separator(&mut spans);
        spans.push(Span::raw(format!(
            "{:?} search: {}",
            search.scope, search.query
        )));
    }
    if view.has_newer_messages {
        append_separator(&mut spans);
        spans.push(Span::raw("new messages ↓"));
    }
    if let Some(notice) = &view.notice {
        append_separator(&mut spans);
        spans.push(Span::raw(notice.clone()));
    }
    spans
}

fn append_separator(spans: &mut Vec<Span<'static>>) {
    if !spans.is_empty() {
        spans.push(Span::raw(" · "));
    }
}

pub(in crate::source) fn render_help(
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
    overlays::render_overlay(frame, popup, lines.collect());
}

pub(in crate::source) fn render_folder_picker(frame: &mut Frame<'_>, area: Rect, view: &View) {
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
    overlays::render_overlay(frame, popup, lines.collect());
}

pub(in crate::source) fn selection_rule(selected: bool) -> Span<'static> {
    if selected {
        Span::styled("▌ ", Style::default().fg(PRIMARY))
    } else {
        Span::raw("  ")
    }
}

pub(in crate::source) fn interaction_rule(active: bool) -> Span<'static> {
    if active {
        Span::styled("│ ", Style::default().fg(PRIMARY))
    } else {
        Span::styled("│ ", Style::default().fg(MUTED_TEXT))
    }
}

pub(in crate::source) fn surface_style(focused: bool) -> Style {
    let style = Style::default().fg(TEXT);
    if focused {
        style.bg(FOCUSED_SURFACE_BACKGROUND)
    } else {
        style
    }
}

#[derive(Debug, Default)]
pub(crate) struct ChatViewport {
    start: Option<usize>,
}

impl ChatViewport {
    pub(in crate::source) fn window(
        &mut self,
        length: usize,
        active: Option<usize>,
        visible_items: usize,
        default_to_end: bool,
        direction: ScrollDirection,
    ) -> std::ops::Range<usize> {
        let visible_items = visible_items.max(1).min(length);
        let active = active
            .map(|index| index.min(length.saturating_sub(1)))
            .or_else(|| default_to_end.then(|| length.saturating_sub(1)))
            .unwrap_or(0);
        let max_start = length.saturating_sub(visible_items);
        let mut start = self.start.unwrap_or_else(|| {
            active
                .saturating_sub(anchor(visible_items, direction))
                .min(max_start)
        });
        start = start.min(max_start);
        let end = start.saturating_add(visible_items);
        if active < start || active >= end {
            start = active
                .saturating_sub(anchor(visible_items, direction))
                .min(max_start);
        } else {
            let cap = start.saturating_add(anchor(visible_items, direction));
            match direction {
                ScrollDirection::Up if active < cap => {
                    start = active
                        .saturating_sub(anchor(visible_items, direction))
                        .min(max_start);
                }
                ScrollDirection::Down if active > cap => {
                    start = active
                        .saturating_sub(anchor(visible_items, direction))
                        .min(max_start);
                }
                ScrollDirection::Up | ScrollDirection::Down => {}
            }
        }
        self.start = Some(start);
        start..start.saturating_add(visible_items)
    }
}

const fn anchor(visible_items: usize, direction: ScrollDirection) -> usize {
    let percent = match direction {
        ScrollDirection::Up => 30,
        ScrollDirection::Down => 70,
    };
    visible_items
        .saturating_sub(1)
        .saturating_mul(percent)
        .saturating_add(50)
        / 100
}

pub(in crate::source) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
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
