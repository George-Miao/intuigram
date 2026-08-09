use intuigram_app::{
    RichMediaComposerMode, RichMediaComposerView, RichMediaLibraryKind, RichMediaUploadKind,
};

use super::*;

pub(super) const RICH_MEDIA_BINDINGS: &[Binding] = &[
    binding(
        KeyChord::plain(Key::Enter),
        "Choose",
        Action::ChooseRichMedia,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char(' ')),
        "Change Type",
        Action::CycleRichMediaKind,
        true,
    ),
];

pub(super) fn render_rich_media(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let Some(composer) = &view.rich_media else {
        return;
    };
    let popup = centered_rect(68, 55, area);
    let heading = if composer.pending {
        let label = if matches!(composer.mode, RichMediaComposerMode::PlaceSearch { .. }) {
            "Searching places"
        } else {
            "Loading media"
        };
        Line::from(effort_spans(label, view.animation_frame))
    } else {
        Line::from(Span::styled(
            title(&composer.mode),
            Style::default().add_modifier(Modifier::BOLD),
        ))
    };
    let mut lines = vec![heading, Line::from("")];
    match &composer.mode {
        RichMediaComposerMode::Menu => lines.extend(
            [
                "Sticker",
                "GIF",
                "Custom emoji",
                "Local file",
                "Voice message",
                "Video note",
                "Contact",
                "Location",
                "Place search",
            ]
            .into_iter()
            .enumerate()
            .map(|(index, label)| selected_line(composer.selected == index, label)),
        ),
        RichMediaComposerMode::Library { items, .. } => {
            lines.extend(items.iter().enumerate().map(|(index, item)| {
                selected_line(composer.selected == index, item.label.as_str())
            }));
            if items.is_empty() && !composer.pending {
                lines.push(Line::from(Span::styled(
                    "No saved media found",
                    Style::default().fg(MUTED_TEXT),
                )));
            }
        }
        RichMediaComposerMode::File { path, kind } => {
            lines.push(field_line(composer.selected == 0, "Path", path));
            lines.push(field_line(
                composer.selected == 1,
                "Send as",
                upload_kind(*kind),
            ));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Enter an exact path; Space changes its Telegram media type.",
                Style::default().fg(MUTED_TEXT),
            )));
        }
        RichMediaComposerMode::Recording {
            kind,
            seconds,
            device,
        } => {
            lines.push(field_line(composer.selected == 0, "Seconds", seconds));
            lines.push(field_line(composer.selected == 1, "Device", device));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("Records a {} with ffmpeg", upload_kind(*kind)),
                Style::default().fg(MUTED_TEXT),
            )));
        }
        RichMediaComposerMode::Contact {
            phone,
            first_name,
            last_name,
        } => {
            lines.push(field_line(composer.selected == 0, "Phone", phone));
            lines.push(field_line(composer.selected == 1, "First name", first_name));
            lines.push(field_line(composer.selected == 2, "Last name", last_name));
        }
        RichMediaComposerMode::StaticLocation { input } => {
            lines.push(field_line(composer.selected == 0, "Coordinates", input));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Enter latitude, longitude or a direct Apple, Google, or OpenStreetMap URL.",
                Style::default().fg(MUTED_TEXT),
            )));
        }
        RichMediaComposerMode::PlaceSearch {
            query,
            near,
            results,
        } => {
            lines.push(field_line(composer.selected == 0, "Search", query));
            lines.push(field_line(composer.selected == 1, "Near", near));
            for (index, place) in results.iter().enumerate() {
                lines.push(selected_line(
                    composer.selected == index + 2,
                    &format!("{} · {}", place.title, place.address),
                ));
            }
            if results.is_empty() && !composer.pending {
                lines.push(Line::from(Span::styled(
                    "Near is optional and only accepts an explicit coordinate or direct map URL.",
                    Style::default().fg(MUTED_TEXT),
                )));
            }
        }
    }
    render_overlays::render_overlay(frame, popup, lines);
    render_cursor(frame, popup, composer);
}

fn title(mode: &RichMediaComposerMode) -> &'static str {
    match mode {
        RichMediaComposerMode::Menu => "Send media",
        RichMediaComposerMode::Library { kind, .. } => match kind {
            RichMediaLibraryKind::Stickers => "Saved stickers",
            RichMediaLibraryKind::Gifs => "Saved GIFs",
            RichMediaLibraryKind::CustomEmoji => "Custom emoji",
        },
        RichMediaComposerMode::File { .. } => "Send local file",
        RichMediaComposerMode::Recording { kind, .. } => upload_kind(*kind),
        RichMediaComposerMode::Contact { .. } => "Send contact",
        RichMediaComposerMode::StaticLocation { .. } => "Send location",
        RichMediaComposerMode::PlaceSearch { .. } => "Search places",
    }
}

fn upload_kind(kind: RichMediaUploadKind) -> &'static str {
    match kind {
        RichMediaUploadKind::Photo => "photo",
        RichMediaUploadKind::Video => "video",
        RichMediaUploadKind::File => "file",
        RichMediaUploadKind::Animation => "animation",
        RichMediaUploadKind::Sticker => "sticker",
        RichMediaUploadKind::CustomEmoji => "custom emoji",
        RichMediaUploadKind::Voice => "voice message",
        RichMediaUploadKind::VideoNote => "video note",
    }
}

fn selected_line(selected: bool, text: &str) -> Line<'static> {
    Line::from(vec![selection_rule(selected), Span::raw(text.to_owned())])
}

fn field_line(selected: bool, label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        interaction_rule(selected),
        Span::styled(format!("{label:<12}"), Style::default().fg(MUTED_TEXT)),
        Span::raw(value.to_owned()),
    ])
}

fn render_cursor(frame: &mut Frame<'_>, popup: Rect, composer: &RichMediaComposerView) {
    let value = match (&composer.mode, composer.selected) {
        (RichMediaComposerMode::File { path, .. }, 0) => Some(path),
        (RichMediaComposerMode::Recording { seconds, .. }, 0) => Some(seconds),
        (RichMediaComposerMode::Recording { device, .. }, 1) => Some(device),
        (RichMediaComposerMode::Contact { phone, .. }, 0) => Some(phone),
        (RichMediaComposerMode::Contact { first_name, .. }, 1) => Some(first_name),
        (RichMediaComposerMode::Contact { last_name, .. }, 2) => Some(last_name),
        (RichMediaComposerMode::StaticLocation { input }, 0) => Some(input),
        (RichMediaComposerMode::PlaceSearch { query, .. }, 0) => Some(query),
        (RichMediaComposerMode::PlaceSearch { near, .. }, 1) => Some(near),
        _ => None,
    };
    let Some(value) = value else { return };
    let content = render_overlays::popup_content_area(popup);
    if content.width == 0 || content.height == 0 {
        return;
    }
    let x = content
        .x
        .saturating_add(14)
        .saturating_add(u16::try_from(value.chars().count()).unwrap_or(u16::MAX))
        .min(content.right().saturating_sub(1));
    let y = content
        .y
        .saturating_add(2 + composer.selected as u16)
        .min(content.bottom().saturating_sub(1));
    frame.set_cursor_position((x, y));
}
