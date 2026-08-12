use super::*;

pub(crate) fn normalize_forward(
    forward: Option<&tl::enums::MessageFwdHeader>,
    names: &HashMap<ChatId, String>,
) -> Option<String> {
    let tl::enums::MessageFwdHeader::Header(forward) = forward?;
    forward
        .from_name
        .clone()
        .or_else(|| {
            forward
                .from_id
                .as_ref()
                .map(marked_peer_id)
                .and_then(|id| names.get(&id).cloned())
        })
        .or_else(|| forward.post_author.clone())
        .or_else(|| Some("Unknown source".to_owned()))
}

pub(crate) fn normalize_reactions(
    reactions: Option<&tl::enums::MessageReactions>,
) -> Vec<ReactionView> {
    let Some(tl::enums::MessageReactions::Reactions(reactions)) = reactions else {
        return Vec::new();
    };
    reactions
        .results
        .iter()
        .map(|result| {
            let tl::enums::ReactionCount::Count(result) = result;
            ReactionView {
                label: match &result.reaction {
                    tl::enums::Reaction::Empty => "reaction".to_owned(),
                    tl::enums::Reaction::Emoji(reaction) => reaction.emoticon.clone(),
                    tl::enums::Reaction::CustomEmoji(reaction) => {
                        format!("custom:{}", reaction.document_id)
                    }
                    tl::enums::Reaction::Paid => "⭐".to_owned(),
                },
                count: u32::try_from(result.count).unwrap_or(0),
                chosen: result.chosen_order.is_some(),
            }
        })
        .collect()
}
