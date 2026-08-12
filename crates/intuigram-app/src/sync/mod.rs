//! Durable synchronization boundary between Telegram updates and application
//! state.

mod cursor;
mod message;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub use cursor::store_cursor;
use cursor::{CursorDelta, apply_cursor_delta};
use intuigram_lib::{AdapterEvent, Bootstrap, ChatId, ChatKind, ChatView};
use intuigram_store::{
    AccountStore, DatabaseRequest, StoredChat, StoredFolder, StoredMutation, SyncBatch, SyncCursor,
};
use intuigram_telegram::LiveEvent;
use message::stored_paid_media_items_json;
pub use message::{decode_stored_message, encode_stored_message};
use snafu::{ResultExt, Snafu};

/// Failure while committing normalized Telegram state before exposure.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// Telegram reported or exposed a discontinuity in a durable update scope.
    #[snafu(display("Telegram update gap in {scope}; synchronization must restart"))]
    UpdateGap { scope: String },

    /// The commit could not be submitted to the database worker.
    #[snafu(display("failed to enqueue a durable Telegram update"))]
    EnqueueUpdate { source: intuigram_store::Error },

    /// The normalized records and cursor did not commit atomically.
    #[snafu(display("failed to commit a durable Telegram update"))]
    CommitUpdate { source: intuigram_store::Error },
}

/// Result returned by the durable synchronization boundary.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Reports whether a fresh Telegram bootstrap can reconcile this failure.
    #[must_use]
    pub const fn requires_reconnect(&self) -> bool {
        matches!(self, Self::UpdateGap { .. })
    }
}

/// Single-owner gate that advances a synchronization cursor only with its
/// normalized durable records.
pub struct UpdateCommitter {
    store: AccountStore,
    cursors: HashMap<String, SyncCursor>,
    known_chats: HashSet<ChatId>,
}
pub(crate) enum CommitProgress {
    Started(UpdateCommit),
    Deferred(DeferredUpdate),
}

pub(crate) struct DeferredUpdate {
    pub(crate) update: LiveEvent,
    pub(crate) scope: String,
}

impl UpdateCommitter {
    /// Creates a commit gate at the last durable cursor.
    #[must_use]
    pub fn new(
        store: AccountStore,
        cursors: impl IntoIterator<Item = SyncCursor>,
        known_chats: impl IntoIterator<Item = ChatId>,
    ) -> Self {
        Self {
            store,
            cursors: cursors
                .into_iter()
                .map(|cursor| (cursor.scope.clone(), cursor))
                .collect(),
            known_chats: known_chats.into_iter().collect(),
        }
    }

    /// Starts one atomic update commit. The returned future yields adapter
    /// events only after SQLite confirms the records and cursor are durable.
    pub fn commit(&mut self, update: LiveEvent) -> Result<UpdateCommit> {
        match self.commit_or_defer(update)? {
            CommitProgress::Started(request) => Ok(request),
            CommitProgress::Deferred(deferred) => UpdateGapSnafu {
                scope: deferred.scope,
            }
            .fail(),
        }
    }

    pub(crate) fn commit_or_defer(&mut self, mut update: LiveEvent) -> Result<CommitProgress> {
        let had_cursors = !update.cursors.is_empty();
        let mut expose_events = false;
        let mut cursors: Vec<(String, SyncCursor)> = Vec::with_capacity(update.cursors.len());
        for delta in &update.cursors {
            let scope = delta.scope.storage_key();
            let current = cursors
                .iter()
                .rev()
                .find(|(planned_scope, _)| planned_scope == &scope)
                .map_or_else(
                    || {
                        self.cursors.get(&scope).cloned().unwrap_or(SyncCursor {
                            scope: scope.clone(),
                            ..SyncCursor::default()
                        })
                    },
                    |(_, cursor)| cursor.clone(),
                );
            match apply_cursor_delta(current, delta)? {
                CursorDelta::Applied {
                    cursor,
                    expose_events: cursor_exposes_events,
                } => {
                    expose_events |= cursor_exposes_events;
                    cursors.push((scope, cursor));
                }
                CursorDelta::Deferred { scope } => {
                    return Ok(CommitProgress::Deferred(DeferredUpdate { update, scope }));
                }
            }
        }
        update.cursors.clear();
        if had_cursors && !expose_events {
            update.events.clear();
        }
        discover_missing_chats(&mut self.known_chats, &mut update.events);
        let batch = sync_batch_for_event(
            cursors.iter().map(|(_, cursor)| cursor.clone()).collect(),
            &update,
        );
        let request = self.store.commit_sync(batch).context(EnqueueUpdateSnafu)?;
        for (scope, cursor) in cursors {
            self.cursors.insert(scope, cursor);
        }
        Ok(CommitProgress::Started(UpdateCommit {
            request: Box::pin(request),
            events: Some(update.events.into()),
            peers: Some(update.peers),
        }))
    }
}

/// One durable Telegram update ready for adapter-state and application use.
pub struct CommittedUpdate {
    /// Intuigram-owned events that may now be exposed to the application.
    pub events: VecDeque<AdapterEvent>,

    /// Operation addresses learned from the committed update envelope.
    pub peers: intuigram_telegram::PeerDirectory,
}

/// In-flight atomic update commit.
#[must_use = "an update must be awaited before its events can be exposed"]
pub struct UpdateCommit {
    request: Pin<Box<DatabaseRequest<()>>>,
    events: Option<VecDeque<AdapterEvent>>,
    peers: Option<intuigram_telegram::PeerDirectory>,
}

impl Future for UpdateCommit {
    type Output = Result<CommittedUpdate>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.request.as_mut().poll(cx) {
            Poll::Ready(result) => {
                if let Err(error) = result.context(CommitUpdateSnafu) {
                    return Poll::Ready(Err(error));
                }
                Poll::Ready(Ok(CommittedUpdate {
                    events: self.events.take().unwrap_or_default(),
                    peers: self.peers.take().unwrap_or_default(),
                }))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Builds the initial atomic synchronized-cache write for an Account bootstrap.
#[must_use]
pub fn bootstrap_sync_batch(
    bootstrap: &Bootstrap,
    cursors: impl IntoIterator<Item = SyncCursor>,
) -> SyncBatch {
    let active_chat = bootstrap.chats.first().map(|chat| chat.id);
    let chat_order = bootstrap.chats.iter().map(|chat| chat.id.0).collect();
    SyncBatch {
        cursors: cursors.into_iter().collect(),
        folders: bootstrap
            .folders
            .iter()
            .map(|folder| StoredFolder {
                id: folder.id,
                title: folder.title.clone(),
                unread: folder.unread,
            })
            .collect(),
        chats: bootstrap.chats.iter().map(stored_chat).collect(),
        chat_order: Some(chat_order),
        messages: active_chat.map_or_else(Vec::new, |chat| {
            bootstrap
                .messages
                .iter()
                .map(|message| encode_stored_message(chat, message))
                .collect()
        }),
        mutations: Vec::new(),
    }
}

fn sync_batch_for_event(cursors: Vec<SyncCursor>, update: &LiveEvent) -> SyncBatch {
    let messages = update
        .events
        .iter()
        .filter_map(|event| match event {
            AdapterEvent::MessageAdded { chat, message }
            | AdapterEvent::MessageUpdated { chat, message } => {
                Some(encode_stored_message(*chat, message))
            }
            _ => None,
        })
        .collect();
    let chats = update
        .events
        .iter()
        .filter_map(|event| match event {
            AdapterEvent::ChatDiscovered { chat } => Some(stored_chat(chat)),
            _ => None,
        })
        .collect();
    let mutations = update.events.iter().filter_map(stored_mutation).collect();
    SyncBatch {
        cursors,
        folders: Vec::new(),
        chats,
        chat_order: None,
        messages,
        mutations,
    }
}

fn discover_missing_chats(known: &mut HashSet<ChatId>, events: &mut Vec<AdapterEvent>) {
    let pin_permissions = events
        .iter()
        .filter_map(|event| match event {
            AdapterEvent::ChatPinPermissionChanged {
                chat,
                can_pin_messages,
            } => Some((*chat, *can_pin_messages)),
            _ => None,
        })
        .collect::<HashMap<_, _>>();
    let mut normalized = Vec::with_capacity(events.len());
    for event in events.drain(..) {
        let mut discovered = match &event {
            AdapterEvent::MessageAdded { chat, message }
            | AdapterEvent::MessageUpdated { chat, message }
                if known.insert(*chat) =>
            {
                Some(discovered_chat(
                    *chat,
                    message.body.clone(),
                    u32::from(message.direction == intuigram_lib::MessageDirection::Incoming),
                ))
            }
            AdapterEvent::ChatArchiveChanged { chat, archived } if known.insert(*chat) => {
                let mut chat = discovered_chat(*chat, String::new(), 0);
                chat.folders = vec![if *archived { -1 } else { 0 }];
                Some(chat)
            }
            _ => None,
        };
        if let Some(chat) = &mut discovered
            && let Some(can_pin_messages) = pin_permissions.get(&chat.id)
        {
            chat.can_pin_messages = *can_pin_messages;
        }
        if let Some(chat) = discovered {
            normalized.push(AdapterEvent::ChatDiscovered { chat });
        }
        normalized.push(event);
    }
    *events = normalized;
}

fn discovered_chat(id: ChatId, preview: String, unread: u32) -> ChatView {
    ChatView {
        id,
        title: format!("Chat {}", id.0),
        preview,
        preview_sender: None,
        preview_sender_peer: None,
        preview_timestamp: String::new(),
        status: "unavailable".to_owned(),
        unread,
        pinned: false,
        can_pin_messages: false,
        has_topics: false,
        has_direct_messages: false,
        kind: ChatKind::Inaccessible,
        folders: vec![0],
    }
}

fn stored_mutation(event: &AdapterEvent) -> Option<StoredMutation> {
    match event {
        AdapterEvent::ChatPinPermissionChanged {
            chat,
            can_pin_messages,
        } => Some(StoredMutation::SetChatPinPermission {
            chat_id: chat.0,
            can_pin_messages: *can_pin_messages,
        }),
        AdapterEvent::ChatTopicsChanged(availability) => Some(StoredMutation::SetChatHasTopics {
            chat_id: availability.chat.0,
            has_topics: availability.has_topics,
        }),
        AdapterEvent::MessagesPinChanged { chat, ids, pinned } => {
            Some(StoredMutation::SetMessagesPinned {
                chat_id: chat.0,
                ids: ids.iter().map(|id| id.0).collect(),
                pinned: *pinned,
            })
        }
        AdapterEvent::PaidMediaItemsUpdated {
            chat,
            message,
            items,
        } => Some(StoredMutation::SetPaidMediaItems {
            chat_id: chat.0,
            message_id: message.0,
            items: stored_paid_media_items_json(items),
        }),
        AdapterEvent::MessagesDeleted { chat, ids } => Some(StoredMutation::DeleteMessages {
            chat_id: chat.map(|chat| chat.0),
            ids: ids.iter().map(|id| id.0).collect(),
        }),
        AdapterEvent::HistoryRead {
            chat,
            saved_peer,
            max_id,
            outgoing,
            unread,
        } => Some(StoredMutation::ReadHistory {
            chat_id: chat.0,
            saved_peer: saved_peer.map(|peer| peer.0),
            max_id: max_id.0,
            outgoing: *outgoing,
            unread: *unread,
        }),
        AdapterEvent::ChatArchiveChanged { chat, archived } => Some(StoredMutation::MoveArchive {
            chat_id: chat.0,
            archived: *archived,
        }),
        _ => None,
    }
}

fn stored_chat(chat: &ChatView) -> StoredChat {
    StoredChat {
        id: chat.id.0,
        kind: match chat.kind {
            ChatKind::SavedMessages => "saved_messages",
            ChatKind::Private => "private",
            ChatKind::Bot => "bot",
            ChatKind::BasicGroup => "basic_group",
            ChatKind::Supergroup => "supergroup",
            ChatKind::Gigagroup => "gigagroup",
            ChatKind::Channel => "channel",
            ChatKind::Inaccessible => "inaccessible",
        }
        .to_owned(),
        title: chat.title.clone(),
        preview: chat.preview.clone(),
        status: chat.status.clone(),
        unread: chat.unread,
        pinned: chat.pinned,
        can_pin_messages: chat.can_pin_messages,
        has_topics: chat.has_topics,
        has_direct_messages: chat.has_direct_messages,
        folders: chat.folders.clone(),
    }
}
