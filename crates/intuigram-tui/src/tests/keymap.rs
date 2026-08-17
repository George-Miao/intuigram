use super::*;

#[test]
fn message_selection_replaces_actions_key_with_enter() {
    let keymap = EffectiveKeymap::defaults();
    let mut current = view(vec![Action::OpenActions]);
    current.focus = Focus::Transcript;
    current.selected_messages = vec![MessageId(7)];

    assert_eq!(
        keymap.resolve(&current, KeyChord::plain(Key::Enter)),
        Some(Action::OpenActions)
    );
    assert_eq!(
        keymap.resolve(&current, KeyChord::plain(Key::Char('a'))),
        None
    );
    assert_eq!(
        keymap
            .action_bar(&current)
            .find(|binding| binding.action == Action::OpenActions)
            .map(|binding| binding.key),
        Some(KeyChord::plain(Key::Enter))
    );
}
