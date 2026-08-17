use super::*;

pub(super) fn centered_message_line(
    mut prefix: Vec<Span<'static>>,
    content: Vec<Span<'static>>,
    width: u16,
) -> Line<'static> {
    let prefix_width = Line::from(prefix.as_slice()).width();
    let content_width = Line::from(content.as_slice()).width();
    let content_start = usize::from(width).saturating_sub(content_width) / 2;
    prefix.push(Span::raw(
        " ".repeat(content_start.saturating_sub(prefix_width)),
    ));
    prefix.extend(content);
    Line::from(prefix)
}
