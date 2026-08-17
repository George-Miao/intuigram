use super::*;

pub(super) fn overlay_open(view: &View) -> bool {
    view.help_open
        || view.action_menu.is_some()
        || view.image_popup.is_some()
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
        || view.todo_editor.is_some()
        || view.link_confirmation.is_some()
}

pub(super) fn focus_visible(view: &View, focus: Focus) -> bool {
    view.focus == focus && !overlay_open(view)
}
