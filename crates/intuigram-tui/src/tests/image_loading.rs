use super::*;

#[test]
fn image_placeholder_animates_in_the_final_preview_geometry() {
    let mut current = image_message_view();
    let fallback = symbols(&render_test_frame(&current, 100, 40).buffer);
    assert!(fallback.contains("[Photo]"));
    assert!(fallback.contains("image"));

    current.media_preview_loads = vec![intuigram_app::MediaPreviewLoadView {
        chat: ChatId(10),
        message: MessageId(40),
    }];

    let loading = render_test_frame(&current, 100, 40);
    let loading_height = message_height(&loading);
    let loading_text = symbols(&loading.buffer);
    assert!(loading_text.contains("loading image"));
    assert!(loading_text.matches('░').count() > 150);

    current.animation_frame = 1;
    let next = render_test_frame(&current, 100, 40);
    assert_ne!(
        highlighted_columns(&loading.buffer),
        highlighted_columns(&next.buffer)
    );

    current.media_preview_loads.clear();
    current.media_previews = vec![intuigram_app::MediaPreviewView {
        chat: ChatId(10),
        message: MessageId(40),
        image: intuigram_app::InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
            .expect("fixture pixels should match their dimensions"),
    }];
    let ready = render_test_frame(&current, 100, 40);
    assert_eq!(message_height(&ready), loading_height);
    let ready_text = symbols(&ready.buffer);
    assert!(ready_text.contains('▀'));
    assert!(!ready_text.contains("[Photo]"));
    assert!(!ready_text.contains("loading image"));
    assert!(!ready_text.contains('░'));
    let rows = rendered_rows(&ready.buffer);
    let image_row = rows
        .iter()
        .position(|row| row.contains('▀'))
        .expect("inline image should render");
    let caption_row = rows
        .iter()
        .position(|row| row.contains("caption"))
        .expect("caption should render");
    assert!(caption_row > image_row);

    current.messages[0].body = "[Photo] image".to_owned();
    let uncaptioned = symbols(&render_test_frame(&current, 100, 40).buffer);
    assert!(!uncaptioned.contains("[Photo] image"));
}

fn image_message_view() -> View {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram".to_owned(),
        preview: String::new(),
        status: String::new(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.messages = vec![MessageView {
        id: MessageId(40),
        sender: "Lin".to_owned(),
        body: "caption".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails {
            media: Some(MediaCard {
                kind: MediaKind::Photo,
                title: "Photo".to_owned(),
                description: "image".to_owned(),
                details: Vec::new(),
                poll: None,
                remote_id: Some("42".to_owned()),
            }),
            ..MessageDetails::default()
        },
    }];
    current
}

fn message_height(frame: &crate::TestFrame) -> u16 {
    frame
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Message)
        .expect("Message semantics should be present")
        .bounds
        .height
}

fn symbols(buffer: &ratatui::buffer::Buffer) -> String {
    buffer.content.iter().map(|cell| cell.symbol()).collect()
}

fn rendered_rows(buffer: &ratatui::buffer::Buffer) -> Vec<String> {
    (0..buffer.area.height)
        .map(|row| {
            (0..buffer.area.width)
                .map(|column| buffer[(column, row)].symbol())
                .collect()
        })
        .collect()
}

fn highlighted_columns(buffer: &ratatui::buffer::Buffer) -> Vec<u16> {
    (0..buffer.area.width)
        .filter(|column| (0..buffer.area.height).any(|row| buffer[(*column, row)].symbol() == "▒"))
        .collect()
}
