use super::*;

pub(super) fn normalize_story(media: &tl::types::MessageMediaStory) -> MediaCard {
    normalize_story_item(
        marked_peer_id(&media.peer),
        media.id,
        media.story.as_ref(),
        media.via_mention,
    )
}

pub(crate) fn normalize_story_item(
    peer: ChatId,
    id: i32,
    story: Option<&tl::enums::StoryItem>,
    via_mention: bool,
) -> MediaCard {
    let (state, caption, date, expires, close_friends, live) = match story {
        Some(tl::enums::StoryItem::Item(story)) => (
            StoryStateView::Available,
            story.caption.clone(),
            format_date(story.date),
            format_date(story.expire_date),
            story.close_friends,
            false,
        ),
        Some(tl::enums::StoryItem::Skipped(story)) => (
            StoryStateView::Skipped,
            None,
            format_date(story.date),
            format_date(story.expire_date),
            story.close_friends,
            story.live,
        ),
        Some(tl::enums::StoryItem::Deleted(_)) => (
            StoryStateView::Deleted,
            None,
            String::new(),
            String::new(),
            false,
            false,
        ),
        None => (
            StoryStateView::Reference,
            None,
            String::new(),
            String::new(),
            false,
            false,
        ),
    };
    MediaCard {
        kind: MediaKind::Story,
        title: "Shared Story".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::Story(SharedStoryView {
            peer,
            id,
            state,
            caption,
            date,
            expires,
            via_mention,
            close_friends,
            live,
        })),
        remote_id: None,
    }
}

pub(super) fn normalize_todo(media: &tl::types::MessageMediaToDo) -> MediaCard {
    let tl::enums::TodoList::List(todo) = &media.todo;
    let completions = media
        .completions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|completion| {
            let tl::enums::TodoCompletion::Completion(completion) = completion;
            (completion.id, completion)
        })
        .collect::<HashMap<_, _>>();
    let items = todo
        .list
        .iter()
        .map(|item| {
            let tl::enums::TodoItem::Item(item) = item;
            let completion = completions.get(&item.id);
            TodoItemView {
                id: item.id,
                title: text_with_entities(item.title.clone()),
                completed: completion.is_some(),
                completed_by: completion.map(|completion| marked_peer_id(&completion.completed_by)),
                completed_date: completion.map(|completion| format_date(completion.date)),
            }
        })
        .collect();
    MediaCard {
        kind: MediaKind::TodoList,
        title: "TODO list".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::TodoList(TodoListView {
            title: text_with_entities(todo.title.clone()),
            items,
            others_can_append: todo.others_can_append,
            others_can_complete: todo.others_can_complete,
        })),
        remote_id: None,
    }
}
