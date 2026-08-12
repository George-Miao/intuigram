use intuigram_lib::FolderEditorView;

use super::*;

pub(in crate::source) const FOLDER_BINDINGS: &[Binding] = &[
    binding(
        KeyChord::plain(Key::Char('f')),
        "Folder Settings",
        Action::ManageFolderLifecycle,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char('n')),
        "New Folder",
        Action::CreateFolder,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char('e')),
        "Edit Folder",
        Action::EditFolder,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Save",
        Action::SaveFolder,
        true,
    ),
    binding(
        KeyChord::shift(Key::Up),
        "Move Earlier",
        Action::ReorderFolderUp,
        true,
    ),
    binding(
        KeyChord::shift(Key::Down),
        "Move Later",
        Action::ReorderFolderDown,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char('s')),
        "Share Folder",
        Action::ShareFolder,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char('d')),
        "Delete Folder",
        Action::DeleteFolder,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Confirm Delete",
        Action::ConfirmDeleteFolder,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char(' ')),
        "Toggle Rule",
        Action::ToggleFolderRule,
        true,
    ),
];

pub(in crate::source) fn render_folder_manager(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let Some(manager) = &view.folder_manager else {
        return;
    };
    let popup = centered_rect(64, 70, area);
    if let Some(folder) = manager.delete_confirmation {
        let title = view
            .folders
            .iter()
            .find(|candidate| candidate.id == folder.0)
            .map_or("Folder", |candidate| candidate.title.as_str());
        overlays::render_overlay(
            frame,
            popup,
            vec![
                Line::from(Span::styled(
                    format!("Delete {title}?"),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Chats and Messages will be retained.",
                    Style::default().fg(MUTED_TEXT),
                )),
            ],
        );
        return;
    }
    if let Some(editor) = &manager.editor {
        render_folder_editor(frame, popup, editor);
        return;
    }
    let heading = if manager.pending {
        Line::from(effort_spans("Updating folders", view.animation_frame))
    } else {
        Line::from(Span::styled(
            "Folder settings",
            Style::default().add_modifier(Modifier::BOLD),
        ))
    };
    let lines = std::iter::once(heading)
        .chain(std::iter::once(Line::from(Span::styled(
            "Create, edit, reorder, share, or delete custom Folders",
            Style::default().fg(MUTED_TEXT),
        ))))
        .chain(std::iter::once(Line::from("")))
        .chain(
            view.folder_details
                .iter()
                .enumerate()
                .map(|(index, details)| {
                    let title = view
                        .folders
                        .iter()
                        .find(|folder| folder.id == details.id.0)
                        .map_or("Folder", |folder| folder.title.as_str());
                    Line::from(vec![
                        selection_rule(manager.selected == index),
                        Span::raw(title.to_owned()),
                        Span::styled(
                            if details.rules.is_none() {
                                "  shared"
                            } else {
                                ""
                            },
                            Style::default().fg(MUTED_TEXT),
                        ),
                    ])
                }),
        )
        .chain((view.folder_details.is_empty()).then(|| {
            Line::from(Span::styled(
                "No custom Folders",
                Style::default().fg(MUTED_TEXT),
            ))
        }))
        .collect();
    overlays::render_overlay(frame, popup, lines);
}

fn render_folder_editor(frame: &mut Frame<'_>, popup: Rect, editor: &FolderEditorView) {
    let mut lines = vec![
        Line::from(Span::styled(
            if editor.id.is_some() {
                "Edit Folder"
            } else {
                "New Folder"
            },
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            selection_rule(editor.selected == 0),
            Span::styled("Title  ", Style::default().fg(MUTED_TEXT)),
            Span::raw(editor.title.clone()),
        ]),
    ];
    if let Some(rules) = editor.rules {
        lines.push(Line::from(""));
        lines.extend(
            [
                ("Contacts", rules.contacts),
                ("Non-contacts", rules.non_contacts),
                ("Groups", rules.groups),
                ("Channels", rules.broadcasts),
                ("Bots", rules.bots),
                ("Exclude muted", rules.exclude_muted),
                ("Exclude read", rules.exclude_read),
                ("Exclude archived", rules.exclude_archived),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (label, enabled))| {
                Line::from(vec![
                    selection_rule(editor.selected == index + 1),
                    Span::styled(
                        if enabled { "[x] " } else { "[ ] " },
                        Style::default().fg(PRIMARY),
                    ),
                    Span::raw(label),
                ])
            }),
        );
    }
    overlays::render_overlay(frame, popup, lines);
    if editor.selected == 0 {
        let content = overlays::popup_content_area(popup);
        if content.width == 0 || content.height == 0 {
            return;
        }
        let cursor = content
            .x
            .saturating_add(9)
            .saturating_add(u16::try_from(editor.title.chars().count()).unwrap_or(u16::MAX))
            .min(content.right().saturating_sub(1));
        let row = content
            .y
            .saturating_add(2)
            .min(content.bottom().saturating_sub(1));
        frame.set_cursor_position((cursor, row));
    }
}
