use super::*;

pub(super) const fn action_icon(action: Action) -> Option<&'static str> {
    match action {
        Action::OpenActions => Some("⋯"),
        Action::Reply => Some("↩"),
        Action::Edit | Action::EditPrevious | Action::SaveEdit | Action::EditTodoList => Some("✎"),
        Action::Delete | Action::ConfirmDelete | Action::CancelOutbox | Action::DismissOutbox => {
            Some("×")
        }
        Action::Forward | Action::ConfirmForward => Some("↪"),
        Action::React | Action::ConfirmReaction => Some("♡"),
        Action::VotePoll | Action::ConfirmPollVote | Action::ResolveOutbox => Some("✓"),
        Action::TogglePollChoice | Action::ToggleTodoItem | Action::ToggleMessageSelection => {
            Some("□")
        }
        Action::RefreshSpecialized | Action::RetryOutbox => Some("↻"),
        Action::OpenLink
        | Action::ConfirmOpenLink
        | Action::OpenImage
        | Action::SaveAs
        | Action::ConfirmSaveAs
        | Action::OpenDownload => Some("↗"),
        Action::DownloadMedia => Some("↓"),
        Action::OpenThread => Some("#"),
        Action::NavigatePinned | Action::TogglePin => Some("⌖"),
        Action::ChooseAction => Some("✓"),
        _ => None,
    }
}

pub(super) fn action_label_width(action: Action, label: &str) -> usize {
    let label_width = Span::raw(label).width();
    action_icon(action).map_or(label_width, |icon| {
        Span::raw(icon).width() + 1 + label_width
    })
}

pub(super) fn push_action_label<'a>(spans: &mut Vec<Span<'a>>, action: Action, label: &'a str) {
    if let Some(icon) = action_icon(action) {
        spans.push(Span::styled(icon, Style::default().fg(PRIMARY)));
        spans.push(Span::raw(" "));
    }
    spans.push(Span::raw(label));
}
