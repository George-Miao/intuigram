pub(super) fn render_delete_confirmation(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let Some(messages) = &view.delete_confirmation else {
        return;
    };
    let popup = centered_rect(48, 28, area);
    let lines = vec![
        Line::from(Span::styled(
            if messages.len() == 1 {
                format!("Delete Message {}?", messages[0].0)
            } else {
                format!("Delete {} Messages?", messages.len())
            },
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "This removes the Message from Telegram.",
            Style::default().fg(MUTED_TEXT),
        )),
    ];
    render_overlay(frame, popup, lines);
}

pub(super) fn render_link_confirmation(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let Some(link) = &view.link_confirmation else {
        return;
    };
    let popup = centered_rect(68, 38, area);
    let lines = vec![
        Line::from(Span::styled(
            "Confirm link destination",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Shown as  ", Style::default().fg(MUTED_TEXT)),
            Span::raw(link.display.clone()),
        ]),
        Line::from(vec![
            Span::styled("Opens     ", Style::default().fg(MUTED_TEXT)),
            Span::styled(link.url.clone(), Style::default().fg(PRIMARY)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "The displayed address is disguised, insecure, or otherwise unusual.",
            Style::default().fg(MUTED_TEXT),
        )),
    ];
    render_overlay(frame, popup, lines);
}

pub(super) fn render_forward_picker(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let messages = if view.selected_messages.is_empty() {
        view.active_message
            .and_then(|index| view.messages.get(index))
            .map(|message| vec![message.id])
            .unwrap_or_default()
    } else {
        view.selected_messages.clone()
    };
    if messages.is_empty() {
        return;
    }
    let source = view
        .active_chat
        .and_then(|index| view.chats.get(index))
        .map(|chat| chat.id);
    let popup = centered_rect(52, 60, area);
    let lines = std::iter::once(Line::from(Span::styled(
        if messages.len() == 1 {
            format!("Forward Message {}", messages[0].0)
        } else {
            format!("Forward {} Messages", messages.len())
        },
        Style::default().add_modifier(Modifier::BOLD),
    )))
    .chain(std::iter::once(Line::from(Span::styled(
        "Choose a destination Chat",
        Style::default().fg(MUTED_TEXT),
    ))))
    .chain(std::iter::once(Line::from("")))
    .chain(
        view.chats
            .iter()
            .enumerate()
            .filter(|(_, chat)| Some(chat.id) != source)
            .map(|(index, chat)| {
                Line::from(vec![
                    selection_rule(view.forward_picker == Some(index)),
                    Span::raw(chat.title.clone()),
                ])
            }),
    )
    .collect();
    render_overlay(frame, popup, lines);
}

pub(super) fn render_reaction_picker(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let Some(picker) = &view.reaction_picker else {
        return;
    };
    let Some(message) = view
        .active_message
        .and_then(|index| view.messages.get(index))
        .map(|message| message.id)
    else {
        return;
    };
    let popup = centered_rect(42, 42, area);
    let lines = std::iter::once(Line::from(Span::styled(
        format!("React to Message {}", message.0),
        Style::default().add_modifier(Modifier::BOLD),
    )))
    .chain(std::iter::once(Line::from("")))
    .chain(picker.options.iter().enumerate().map(|(index, reaction)| {
        Line::from(vec![
            selection_rule(picker.selected == index),
            Span::raw(reaction.clone()),
        ])
    }))
    .collect();
    render_overlay(frame, popup, lines);
}

pub(super) fn render_poll_vote(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let Some(picker) = &view.poll_vote else {
        return;
    };
    let popup = centered_rect(56, 56, area);
    let instruction = if picker.multiple_choice {
        "Space toggles choices · Enter submits"
    } else {
        "Choose an answer · Enter submits"
    };
    let lines = std::iter::once(Line::from(Span::styled(
        format!("Vote in Message {}", picker.message.0),
        Style::default().add_modifier(Modifier::BOLD),
    )))
    .chain(std::iter::once(Line::from(Span::styled(
        instruction,
        Style::default().fg(MUTED_TEXT),
    ))))
    .chain(std::iter::once(Line::from("")))
    .chain(picker.options.iter().enumerate().map(|(index, option)| {
        let marker = if picker.choices.contains(&index) {
            "● "
        } else {
            "○ "
        };
        Line::from(vec![
            selection_rule(picker.selected == index),
            Span::styled(marker, Style::default().fg(PRIMARY)),
            Span::raw(option.clone()),
        ])
    }))
    .collect();
    render_overlay(frame, popup, lines);
}

pub(super) fn render_overlay(frame: &mut Frame<'_>, area: Rect, lines: Vec<Line<'_>>) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines)
            .style(surface_style(true))
            .wrap(Wrap { trim: false }),
        area,
    );
}
use super::*;

pub(super) fn render_attachment_path(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let Some(attachment) = &view.attachment_path else {
        return;
    };
    let popup = centered_rect(68, 28, area);
    let lines = vec![
        Line::from(Span::styled(
            "Attach local file",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            interaction_rule(true),
            Span::raw(attachment.path.clone()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter an exact path; no shell expansion is performed.",
            Style::default().fg(MUTED_TEXT),
        )),
    ];
    render_overlay(frame, popup, lines);
    let cursor_x = popup
        .x
        .saturating_add(2)
        .saturating_add(u16::try_from(attachment.path.chars().count()).unwrap_or(u16::MAX))
        .min(popup.right().saturating_sub(1));
    frame.set_cursor_position((cursor_x, popup.y.saturating_add(2)));
}

pub(super) fn render_save_as(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let Some(save_as) = &view.save_as else {
        return;
    };
    let popup = centered_rect(68, 28, area);
    let lines = vec![
        Line::from(Span::styled(
            "Save media as",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            interaction_rule(true),
            Span::raw(save_as.destination.clone()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "An existing file will not be replaced.",
            Style::default().fg(MUTED_TEXT),
        )),
    ];
    render_overlay(frame, popup, lines);
    let cursor_x = popup
        .x
        .saturating_add(2)
        .saturating_add(u16::try_from(save_as.destination.chars().count()).unwrap_or(u16::MAX))
        .min(popup.right().saturating_sub(1));
    frame.set_cursor_position((cursor_x, popup.y.saturating_add(2)));
}
