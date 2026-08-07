# Intuigram

Intuigram is a terminal interface for using Telegram fluently as a primary client.

## Language

**Daily Driver**:
A primary Telegram client that supports routine communication fluently, including major chat-management functions and the message types people commonly encounter. Calls are currently outside this promise.
_Avoid_: Minimal client, text-only client, companion client

**Folder**:
A user-defined Telegram grouping that determines which Chats appear together. The Active Folder scopes the Chat list without becoming an interaction target; All Chats and Archive use the same navigation concept.
_Avoid_: Workspace, category, tab

**Message**:
An item in a Chat whose content may be text, media, a file, a poll, a location, a contact, or a Telegram service event.
_Avoid_: Text message

**Message History**:
The complete chronological sequence of Messages in a Chat, loaded incrementally without an artificial recent-message limit.
_Avoid_: Latest page, 100-message snapshot

**Active Message**:
The single Message temporarily targeted by navigation and compatible Current Actions within the Active Chat. Activating a Message transfers interaction from the Composer to the Transcript until the target is cleared.
_Avoid_: Selected Message, Read Message, newest Message

**Message Selection**:
One or more explicitly selected Messages targeted together by compatible Current Actions.
_Avoid_: Active Message, visible Messages

**Chat**:
A Telegram conversation and the container for its Messages.
_Avoid_: Conversation, thread

**Active Chat**:
The single Chat currently shown in the Transcript and targeted by Chat-level actions. Moving through the Chat list changes the Active Chat immediately, independently of whether the Chat list or the Chat itself has focus.
_Avoid_: Selected Chat, current Chat, preview Chat

**Private Chat**:
A Chat whose peer is a human user, bot, or the current Account itself.
_Avoid_: Direct Message when referring to the Chat itself

**Secret Chat**:
A device-specific, end-to-end encrypted Telegram conversation that is not part of the Telegram cloud. Secret Chats are explicitly outside Intuigram's Daily Driver promise; they must not be represented as ordinary Private Chats or partially implemented without a complete, reviewed security design.
_Avoid_: Private Chat, cloud Chat, encrypted-at-rest Chat

**Basic Group**:
A legacy Telegram group with a `peerChat` identity and a limited feature set.
_Avoid_: Supergroup

**Supergroup**:
A group with a `peerChannel` identity and Telegram's full group feature set.
_Avoid_: Basic Group, Channel

**Gigagroup**:
A Supergroup large enough that only administrators can post.
_Avoid_: Channel

**Channel**:
A broadcast Chat where administrators publish and subscribers primarily consume.
_Avoid_: Supergroup

**Topic**:
A nested Message history within a forum Supergroup or topic-enabled bot Private Chat.
_Avoid_: Chat, Folder, generic thread

**Thread**:
A nested reply history rooted at a Message. Channel comments are Threads backed by the Channel's linked discussion Supergroup.
_Avoid_: Topic, Chat

**Account**:
A Telegram user identity, identified in storage by its unique Telegram user ID.
_Avoid_: Local UUID, profile

**Read State**:
Telegram's acknowledgement that incoming Messages have been seen. Intuigram advances Read State only when the Chat has focus and its newest Message is visible, not when the Chat is selected, previewed, or synchronized.
_Avoid_: Loaded state, selected state

**Notification**:
An operating-system alert for an incoming Message outside the focused Chat, subject to Telegram mute settings and Intuigram's privacy preference. A terminal bell is the fallback when desktop integration is unavailable.
_Avoid_: In-app status message

**Unsupported Content**:
Message content that Intuigram cannot present natively but still represents with an explicit, informative placeholder.
_Avoid_: Empty message, omitted message

**Media Card**:
A Message presentation that communicates a media item's identity, metadata, state, and available actions. It may include inline terminal graphics, but remains useful through its text fallback.
_Avoid_: Attachment placeholder

**Transcript**:
The dense chronological presentation of a Chat's Messages. Consecutive Messages are visually grouped, while sender accents and delivery markers provide distinction without chat bubbles.
_Avoid_: Chat bubbles, message bubbles

**Responsive Hierarchy**:
Normal and wide terminals show the Chat list beside the Active Chat. Narrow terminals show the current hierarchy level: Chat-list interaction presents Chats, while Composer or Active-Message interaction presents the Active Chat. Resizing changes only this projection and preserves the Active Folder, Active Chat, Active Message, anchored history, Draft, and interaction target.
_Avoid_: Resize reset, independent mobile mode

**Details**:
Secondary content for the active Chat, such as information, members, shared media, or search results. Details appear as a third pane only when requested and space permits, otherwise they participate in the same navigation stack as the Chat list and Transcript.
_Avoid_: Permanent sidebar, empty third pane

**Chat Search**:
Search for Messages within the Active Chat, invoked with `Ctrl+F` from anywhere inside that Chat.
_Avoid_: Global Search

**Global Search**:
Search for Chats and Messages across the active Account, invoked with `Ctrl+F` from the Chat list.
_Avoid_: Chat Search

**Draft**:
The durable, Telegram-synchronized unsent content for a Chat. Concurrent local and remote changes resolve by last writer, except that active typing is never replaced in place.
_Avoid_: Composer buffer

**Composer**:
The Chat input surface associated with the Active Chat's Draft and pending reply or attachment context.
_Avoid_: Draft bar, input bar, text box

**Draft History**:
A small local recovery history containing Draft versions displaced during synchronization.
_Avoid_: Conflict prompt, second active Draft

**Synchronized Cache**:
The local representation of Telegram data that is immediately usable at startup and continuously reconciled with Telegram while connected.
_Avoid_: Offline snapshot, manual refresh

**Local Record**:
Durable Account data that Intuigram retains until the user explicitly clears Account data or logs out, including synchronized Chat metadata, Message text, Drafts, Draft History, search data, and operation state.
_Avoid_: Media Cache, temporary file

**Media Cache**:
Size-bounded, redownloadable media and thumbnails stored locally for responsive presentation. Eviction may remove cached bytes but never the corresponding Message or its metadata.
_Avoid_: Local Record, Message History

**Local Lock**:
Optional full-database encryption for one Account's Telegram authorization and Local Records. Unlock material comes from a hidden passphrase or an operating-system credential vault and never enters application state, logs, or configuration.
_Avoid_: Telegram 2FA, screen lock, Media Cache encryption

**Clipboard Paste**:
A context-sensitive composer action that queries the platform clipboard directly. Text is inserted into the Draft, images become photo attachment candidates, and copied files become file attachment candidates; unavailable or unsupported clipboard formats fail visibly without altering the Draft.
_Avoid_: Terminal text paste only, shell evaluation, path picker

**Logout**:
A verified server-side revocation of an Account's current Telegram authorization, followed by deletion of that Account's local session, Local Records, and Media Cache. It succeeds only after Telegram acknowledges revocation.
_Avoid_: Remove locally, clear media cache, exit

**Remove Locally**:
A destructive deletion of an Account's local session, Local Records, and Media Cache without claiming that Telegram revoked the server-side authorization. It requires a warning that the authorization may need termination from another Telegram client.
_Avoid_: Logout, clear media cache

**Rebuild Cache**:
An explicit recovery operation that replaces only redownloadable synchronized Account data while preserving authorization, Drafts, Draft History, configuration, and other unique Local Records. If preservation cannot be guaranteed, Intuigram must require export or explicit abandonment instead of presenting the operation as a safe rebuild.
_Avoid_: Delete Account, Logout, automatic database recreation

**Reconnect Cooldown**:
A disconnected state in which automatic reconnection is deliberately paused after unsuccessful attempts. Only in this state does Intuigram offer an explicit Reconnect action.
_Avoid_: Any transient connection loss

**Pending Action**:
An optimistic Telegram operation accepted while Intuigram is running but not yet acknowledged by Telegram. It remains visibly pending and is retried automatically after reconnection, but is not promised to survive process exit or a crash.
_Avoid_: Draft, durable Outbox, completed action

**Outbox**:
A future durable record of Pending Actions that survives process exit and preserves enough intent to retry or resolve them safely.
_Avoid_: Draft, in-memory request queue

**Scheduled Message**:
A Message submitted to Telegram for future delivery at a specified time or, where supported, when the recipient is online. It is managed through a Chat's scheduled-message history and survives Intuigram exiting.
_Avoid_: Draft, Pending Action, Outbox item

**Current Action**:
An operation available for the focused item in the active view.
_Avoid_: Global shortcut

**Action Bar**:
A persistent summary of all important Current Actions and their keys. Complete and uncommon bindings belong in Help rather than the Action Bar.
_Avoid_: Status bar, exhaustive key list

**Help**:
The complete, context-sensitive reference for bindings available from the active view.
_Avoid_: Action Bar
