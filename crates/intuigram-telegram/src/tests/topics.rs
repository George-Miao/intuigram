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
