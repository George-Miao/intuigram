use super::super::*;
use super::LoginPrompt;

pub(super) fn render_login(frame: &mut Frame<'_>, prompt: &LoginPrompt<'_>, value: &str) {
    let area = centered_rect(64, 56, frame.area());
    let content = Rect::new(
        area.x.saturating_add(2),
        area.y.saturating_add(1),
        area.width.saturating_sub(4),
        area.height.saturating_sub(2),
    );
    let shown = if prompt.secret {
        "•".repeat(value.chars().count())
    } else {
        value.to_owned()
    };
    let mut lines = vec![
        Line::from(Span::styled(
            "Connect Intuigram",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("Step {} of 5", prompt.field.position()),
            Style::default().fg(MUTED_TEXT),
        )),
        Line::from(""),
        Line::from(Span::styled(
            prompt.label.to_owned(),
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![interaction_rule(true), Span::raw(shown)]),
    ];
    if !prompt.description.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            prompt.description.to_owned(),
            Style::default().fg(MUTED_TEXT),
        )));
    }
    if let Some(error) = prompt.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            error.to_owned(),
            Style::default().fg(Color::Red),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(if prompt.can_go_back {
        "Enter Continue · Shift+Tab Back · Esc Cancel"
    } else {
        "Enter Continue · Esc Cancel"
    }));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), content);
    let input_y = content.y.saturating_add(4);
    let input_x = content
        .x
        .saturating_add(2)
        .saturating_add(u16::try_from(value.chars().count()).unwrap_or(u16::MAX))
        .min(content.right().saturating_sub(1));
    frame.set_cursor_position((input_x, input_y));
}
