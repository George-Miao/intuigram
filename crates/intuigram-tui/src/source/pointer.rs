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
        SemanticRole::Message => node
            .domain_id
            .map(|message| ActivationTarget::Message(MessageId(message))),
        SemanticRole::Composer => Some(ActivationTarget::Composer),
        SemanticRole::ChatList
        | SemanticRole::Transcript
        | SemanticRole::MediaCard
        | SemanticRole::Action => None,
    }
}

const fn scroll_target(role: SemanticRole) -> Option<ScrollTarget> {
    match role {
        SemanticRole::ChatList | SemanticRole::Chat => Some(ScrollTarget::Chats),
        SemanticRole::Transcript | SemanticRole::Message | SemanticRole::MediaCard => {
            Some(ScrollTarget::Transcript)
        }
        SemanticRole::Composer | SemanticRole::Folder | SemanticRole::Action => None,
    }
}

fn overlay_open(view: &View) -> bool {
    view.help_open
        || view.action_menu.is_some()
        || view.scheduled.is_some()
        || view.rich_media.is_some()
        || view.attachment_path.is_some()
        || view.save_as.is_some()
        || view.folder_picker.is_some()
        || view.folder_manager.is_some()
        || view.account_picker.is_some()
        || view.account_confirmation.is_some()
        || view.delete_confirmation.is_some()
        || view.forward_picker.is_some()
        || view.reaction_picker.is_some()
        || view.poll_vote.is_some()
        || view.link_confirmation.is_some()
}
