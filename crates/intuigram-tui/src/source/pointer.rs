use super::*;

pub(super) fn resolve_pointer(
    view: &View,
    mouse: crossterm::event::MouseEvent,
    semantics: &[SemanticNode],
) -> Option<UiEvent> {
    if mouse.modifiers != KeyModifiers::NONE || overlay_open(view) {
        return None;
    }
    let node = semantics
        .iter()
        .rev()
        .find(|node| contains(node.bounds, mouse.column, mouse.row))?;
    match mouse.kind {
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            let direction = if mouse.kind == MouseEventKind::ScrollUp {
                ScrollDirection::Up
            } else {
                ScrollDirection::Down
            };
            scroll_target(node.role)
                .map(|target| Intent::Scroll(target, direction))
                .map(UiEvent::Intent)
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(action) = (node.role == SemanticRole::Action)
                .then_some(node.action)
                .flatten()
            {
                return Some(UiEvent::Intent(Intent::Action(action)));
            }
            if node.role == SemanticRole::Composer
                && let Some(cursor) = composer_cursor_at(view, node.bounds, mouse.column, mouse.row)
            {
                return Some(UiEvent::Intent(Intent::SetComposerCursor(cursor)));
            }
            activation_target(node)
                .map(Intent::Activate)
                .map(UiEvent::Intent)
        }
        _ => None,
    }
}

fn contains(bounds: Rect, column: u16, row: u16) -> bool {
    column >= bounds.x && column < bounds.right() && row >= bounds.y && row < bounds.bottom()
}

fn activation_target(node: &SemanticNode) -> Option<ActivationTarget> {
    match node.role {
        SemanticRole::Folder => node
            .domain_id
            .and_then(|folder| i32::try_from(folder).ok())
            .map(ActivationTarget::Folder),
        SemanticRole::Chat => node
            .domain_id
            .map(|chat| ActivationTarget::Chat(ChatId(chat))),
        SemanticRole::Topic => node
            .domain_id
            .map(|topic| ActivationTarget::Topic(TopicId(topic))),
        SemanticRole::SavedDialog => node
            .domain_id
            .map(|peer| ActivationTarget::SavedDialog(ChatId(peer))),
        SemanticRole::Message => node
            .domain_id
            .map(|message| ActivationTarget::Message(MessageId(message))),
        SemanticRole::MediaCard if node.action == Some(Action::OpenImage) => node
            .domain_id
            .map(|message| ActivationTarget::MessageImage(MessageId(message))),
        SemanticRole::Composer => Some(ActivationTarget::Composer),
        SemanticRole::ChatList
        | SemanticRole::TopicList
        | SemanticRole::SavedDialogList
        | SemanticRole::Transcript
        | SemanticRole::MediaCard
        | SemanticRole::Action => None,
    }
}

const fn scroll_target(role: SemanticRole) -> Option<ScrollTarget> {
    match role {
        SemanticRole::ChatList | SemanticRole::Chat => Some(ScrollTarget::Chats),
        SemanticRole::TopicList | SemanticRole::Topic => Some(ScrollTarget::Topics),
        SemanticRole::SavedDialogList | SemanticRole::SavedDialog => {
            Some(ScrollTarget::SavedDialogs)
        }
        SemanticRole::Transcript | SemanticRole::Message | SemanticRole::MediaCard => {
            Some(ScrollTarget::Transcript)
        }
        SemanticRole::Composer | SemanticRole::Folder | SemanticRole::Action => None,
    }
}
