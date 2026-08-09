use super::*;

pub(super) fn message(id: i64, body: &str) -> MessageView {
    MessageView {
        id: MessageId(id),
        sender: "Ferris".to_owned(),
        body: body.to_owned(),
        timestamp: "12:30".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails::default(),
    }
}

pub(super) fn load_chat(chat: i64, selected_chat: Option<i64>) -> Effect {
    Effect::LoadChat {
        chat: ChatId(chat),
        selection: selected_chat.map(|chat| SelectionView {
            folder: 0,
            chat: Some(ChatId(chat)),
            message: None,
        }),
        transcript_anchors: Vec::new(),
    }
}

pub(super) fn assert_jump_adopts_refresh(app: &mut App, refreshed: Vec<MessageView>) {
    let jumped = app.transition(Input::Intent(Intent::Action(Action::JumpLatest)));
    assert_eq!(jumped.view.messages, refreshed);
    assert_eq!(jumped.view.active_message, Some(1));
    assert!(!jumped.view.has_newer_messages);
}
