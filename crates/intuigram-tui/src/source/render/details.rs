use super::*;

pub(in crate::source) fn render_thread_details(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    options: ViewOptions,
    semantics: &mut Vec<SemanticNode>,
    graphics: &mut GraphicsFrame,
    chat_viewport: &mut ChatViewport,
) -> bool {
    if area.width < 140 || view.active_thread.is_none() || view.active_topic.is_some() {
        return false;
    }
    let columns = Layout::horizontal([
        Constraint::Length(40),
        Constraint::Length(1),
        Constraint::Ratio(1, 2),
        Constraint::Length(1),
        Constraint::Ratio(1, 2),
    ])
    .split(area);
    layout::render_chats(
        frame,
        columns[0],
        view,
        options.mode,
        semantics,
        graphics,
        chat_viewport,
    );
    render_parent(frame, columns[2], view, options, graphics);
    layout::render_active_chat(frame, columns[4], view, options, semantics, graphics);
    true
}

fn render_parent(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    options: ViewOptions,
    graphics: &mut GraphicsFrame,
) {
    let mode = options.mode;
    let rows = Layout::vertical([
        Constraint::Length(mode.active_chat_header_height()),
        Constraint::Min(1),
    ])
    .split(area);
    let mut parent = view.clone();
    parent.messages.clone_from(&view.parent_messages);
    parent.active_thread = None;
    parent.active_message = None;
    parent.transcript_anchor = None;
    parent.unread_boundary = None;
    parent.selected_messages.clear();
    render_active_chat_header(frame, rows[0], &parent, mode, false, graphics);
    render_transcript(
        frame,
        rows[1],
        &parent,
        false,
        options,
        &mut Vec::new(),
        graphics,
    );
}
