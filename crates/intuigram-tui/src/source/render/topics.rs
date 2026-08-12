use super::*;
use crate::source::render::text::capped_text;

pub(in crate::source) fn render_topics(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    let focused = view.focus == Focus::Topics;
    frame.render_widget(Paragraph::new("").style(surface_style(focused)), area);
    let area = mode.horizontally_padded(area);
    semantics.push(SemanticNode {
        role: SemanticRole::TopicList,
        name: "Topics".to_owned(),
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
    if view.topics.is_empty() {
        let text = if view.topics_loading {
            Line::from(effort_spans(
                "⌁ intuigram · finding Topics",
                view.animation_frame,
            ))
        } else {
            Line::from(Span::styled("No Topics", Style::default().fg(MUTED_TEXT)))
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
    let active = view.active_topic.unwrap_or(0).min(view.topics.len() - 1);
    let start = active
        .saturating_sub(visible / 2)
        .min(view.topics.len().saturating_sub(visible));
    let end = (start + visible).min(view.topics.len());
    semantics.extend(
        view.topics[start..end]
            .iter()
            .enumerate()
            .map(|(offset, topic)| {
                let index = start + offset;
                SemanticNode {
                    role: SemanticRole::Topic,
                    name: topic.title.clone(),
                    description: Some(topic.preview.clone()),
                    domain_id: Some(topic.id.0),
                    action: None,
                    delivery: None,
                    active: view.active_topic == Some(index),
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
    let items = view.topics[start..end]
        .iter()
        .enumerate()
        .map(|(offset, topic)| {
            let selected = view.active_topic == Some(start + offset);
            let marker = if topic.pinned { " ●" } else { "" };
            let closed = if topic.closed { " closed" } else { "" };
            let unread = if topic.unread > 0 {
                format!(" {}", topic.unread)
            } else {
                String::new()
            };
            let metadata = format!("{}{}{}{}", topic.timestamp, marker, closed, unread);
            let rule_width = Line::from(selection_rule(selected)).width();
            let title_width = usize::from(area.width)
                .saturating_sub(rule_width)
                .saturating_sub(Line::from(metadata.as_str()).width())
                .saturating_sub(1);
            let title = capped_text(&topic.title, title_width);
            let gap = usize::from(area.width)
                .saturating_sub(rule_width)
                .saturating_sub(Line::from(title.as_str()).width())
                .saturating_sub(Line::from(metadata.as_str()).width());
            let mut lines = vec![Line::from(vec![
                selection_rule(selected),
                Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" ".repeat(gap)),
                Span::styled(metadata, Style::default().fg(MUTED_TEXT)),
            ])];
            lines.push(Line::from(vec![
                selection_rule(selected),
                Span::styled(
                    capped_text(
                        &topic.preview.replace('\n', " "),
                        usize::from(area.width).saturating_sub(rule_width),
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
