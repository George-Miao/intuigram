use super::*;

#[test]
fn forum_supergroups_and_topic_enabled_bots_expose_topic_navigation() {
    let mut forum = channel(false, false);
    forum.forum = true;
    let mut bot = user(77, false, true);
    bot.bot_forum_view = true;
    let traits = chat_traits(
        &[tl::enums::Chat::Channel(forum.clone())],
        &[tl::enums::User::User(bot)],
        None,
    );

    assert!(
        traits
            .get(&ChatId(-1_000_000_000_000 - forum.id))
            .is_some_and(|traits| traits.has_topics)
    );
    assert!(
        traits
            .get(&ChatId(77))
            .is_some_and(|traits| traits.has_topics)
    );
}

#[test]
fn only_managed_monoforums_expose_direct_message_dialogs() {
    let mut managed = channel(false, false);
    managed.monoforum = true;
    managed.creator = true;
    let managed_id = ChatId(-1_000_000_000_000 - managed.id);
    let mut ordinary = channel(false, false);
    ordinary.id = 8;
    ordinary.monoforum = true;
    let ordinary_id = ChatId(-1_000_000_000_000 - ordinary.id);

    let traits = chat_traits(
        &[
            tl::enums::Chat::Channel(managed),
            tl::enums::Chat::Channel(ordinary),
        ],
        &[],
        None,
    );

    assert!(traits[&managed_id].has_direct_messages);
    assert!(!traits[&ordinary_id].has_direct_messages);
}
