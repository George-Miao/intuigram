use super::*;

#[test]
fn a_full_effect_queue_fails_instead_of_blocking_terminal_input() {
    let mut pending = VecDeque::from(vec![
        AdapterEffect {
            effect: Effect::Reconnect,
            random_id: None,
            cancellation: Default::default(),
        };
        EFFECT_CAPACITY
    ]);
    let active = futures_util::stream::FuturesUnordered::<PendingEffect>::new();

    let error = enqueue_effect(&mut pending, &active, &[], Some(Effect::Reconnect))
        .expect_err("a saturated effect queue should be reported");

    assert!(matches!(error, Error::EffectsFull { .. }));
}

#[test]
fn rapid_selection_saves_keep_only_the_latest_request() {
    let active = futures_util::stream::FuturesUnordered::<PendingEffect>::new();
    let mut pending = VecDeque::new();

    for chat in 1..=(EFFECT_CAPACITY as i64 + 1) {
        enqueue_effect(
            &mut pending,
            &active,
            &[],
            Some(Effect::SaveSelection {
                folder: 0,
                chat: Some(ChatId(chat)),
                message: None,
                transcript_anchors: Vec::new(),
            }),
        )
        .expect("rapid Chat navigation should coalesce durable selection writes");
    }

    assert_eq!(pending.len(), 1);
    assert!(matches!(
        &pending[0].effect,
        Effect::SaveSelection {
            chat: Some(ChatId(chat)),
            ..
        } if *chat == EFFECT_CAPACITY as i64 + 1
    ));
}

#[test]
fn reconnect_history_retry_keeps_only_the_latest_chat_request() {
    let active = futures_util::stream::FuturesUnordered::<PendingEffect>::new();
    let mut pending = VecDeque::new();

    for chat in [10, 20, 30] {
        enqueue_effect(
            &mut pending,
            &active,
            &[],
            Some(Effect::LoadChat {
                chat: ChatId(chat),
                selection: None,
                transcript_anchors: Vec::new(),
            }),
        )
        .expect("a replacement history request should remain admissible");
    }

    assert_eq!(pending.len(), 1);
    assert!(matches!(
        &pending[0].effect,
        Effect::LoadChat {
            chat: ChatId(30),
            ..
        }
    ));
}
