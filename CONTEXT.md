# Intuigram

Intuigram is a terminal interface for Telegram. It is designed for fluent use as a primary client.

## Language

**Daily Driver**: A primary Telegram client that supports routine communication. It includes important Chat-management functions and common Message types. Calls are outside the current promise. _Avoid_: Minimal client, text-only client, companion client

**Folder**: A user-defined Telegram group that controls which Chats appear together. The Active Folder limits the Chat list. It is not an interaction target. All Chats and Archive use the same navigation concept. _Avoid_: Workspace, category, tab

**Message**: An item in a Chat. Its content can be text, media, a file, a poll, a location, a contact, or a Telegram service event. _Avoid_: Text message

**Message History**: The complete chronological sequence of Messages in a Chat. Intuigram loads it in increments without an artificial limit on recent Messages. _Avoid_: Latest page, 100-message snapshot

**Active Message**: The one Message that navigation and compatible Current Actions temporarily target in the Active Chat. When a Message becomes active, interaction moves from the Composer to the Transcript. Interaction stays there until the target is cleared. _Avoid_: Selected Message, Read Message, newest Message

**Message Selection**: One or more Messages that the user explicitly selects. Compatible Current Actions target them together. _Avoid_: Active Message, visible Messages

**Chat**: A Telegram communication channel that contains its Messages. _Avoid_: Conversation, thread

**Active Chat**: The one Chat that the Transcript shows and Chat-level actions target. Movement through the Chat list immediately changes the Active Chat. This does not depend on whether the Chat list or the Chat has focus. _Avoid_: Selected Chat, current Chat, preview Chat

**Private Chat**: A Chat whose peer is a human user, a bot, or the current Account. _Avoid_: Direct Message when referring to the Chat itself

**Secret Chat**: A device-specific Telegram communication channel with end-to-end encryption. It is not part of the Telegram cloud. Secret Chats are outside the Intuigram Daily Driver promise. Do not represent them as ordinary Private Chats. Do not implement part of the feature without a complete reviewed security design. _Avoid_: Private Chat, cloud Chat, encrypted-at-rest Chat

**Basic Group**: A legacy Telegram group with a `peerChat` identity and a limited feature set. _Avoid_: Supergroup

**Supergroup**: A group with a `peerChannel` identity and the complete Telegram group feature set. _Avoid_: Basic Group, Channel

**Gigagroup**: A Supergroup in which only administrators can post because of its size. _Avoid_: Channel

**Channel**: A broadcast Chat. Administrators publish, and subscribers primarily read. _Avoid_: Supergroup

**Topic**: A nested Message History in a forum Supergroup or a topic-enabled bot Private Chat. _Avoid_: Chat, Folder, generic thread

**Thread**: A nested reply history that starts at a Message. Channel comments are Threads in the linked discussion Supergroup of the Channel. _Avoid_: Topic, Chat

**Account**: A Telegram user identity. Its unique Telegram user ID identifies it in storage. _Avoid_: Local UUID, profile

**Read State**: The Telegram acknowledgement that a user saw incoming Messages. Intuigram advances Read State only when the Chat has focus and its newest Message is visible. It does not advance Read State when the Chat is selected, previewed, or synchronized. _Avoid_: Loaded state, selected state

**Notification**: An operating-system alert for an incoming Message outside the focused Chat. Telegram mute settings and the Intuigram privacy preference control it. A terminal bell is the fallback when desktop integration is not available. _Avoid_: In-app status message

**Unsupported Content**: Message content that Intuigram cannot show in its native form. Intuigram continues to represent it with an explicit informative placeholder. _Avoid_: Empty message, omitted message

**Media Card**: A Message presentation that shows the identity, metadata, state, and available actions of a media item. It can include inline terminal graphics. Its text fallback always stays useful. _Avoid_: Attachment placeholder

**Transcript**: A dense chronological presentation of the Messages in a Chat. It groups consecutive Messages visually. Sender accents and delivery markers distinguish Messages without Chat bubbles. _Avoid_: Chat bubbles, message bubbles

**Responsive Hierarchy**: Normal and wide terminals show the Chat list next to the Active Chat. Narrow terminals show the current hierarchy level. Chat-list interaction shows Chats. Composer or Active-Message interaction shows the Active Chat. A resize changes only this presentation. It preserves the Active Folder, Active Chat, Active Message, anchored history, Draft, and interaction target. _Avoid_: Resize reset, independent mobile mode

**Details**: Secondary content for the Active Chat. Examples are information, members, shared media, and search results. Details appear as a third pane only when the user requests them and sufficient space is available. Otherwise, they use the same navigation stack as the Chat list and Transcript. _Avoid_: Permanent sidebar, empty third pane

**Chat Search**: A search for Messages in the Active Chat. `Ctrl+F` starts it from any location in that Chat. _Avoid_: Global Search

**Global Search**: A search for Chats and Messages in the active Account. `Ctrl+F` starts it from the Chat list. _Avoid_: Chat Search

**Draft**: Durable unsent content for a Chat that synchronizes with Telegram. The last writer resolves concurrent local and remote changes. Active typing is never replaced in place. _Avoid_: Composer buffer

**Composer**: The Chat input surface for the Draft of the Active Chat and its pending reply or attachment context. _Avoid_: Draft bar, input bar, text box

**Draft History**: A small local recovery history. It contains Draft versions that synchronization displaced. _Avoid_: Conflict prompt, second active Draft

**Synchronized Cache**: The local Telegram data that Intuigram can use immediately at startup. Intuigram continuously reconciles it with Telegram while connected. _Avoid_: Offline snapshot, manual refresh

**Local Record**: Durable Account data that Intuigram keeps until the user explicitly clears Account data or logs out. It includes synchronized Chat metadata, Message text, Drafts, Draft History, search data, and operation state. _Avoid_: Media Cache, temporary file

**Media Cache**: Size-bounded redownloadable media and thumbnails that Intuigram stores locally for responsive presentation. Eviction can remove cached bytes. It never removes the related Message or its metadata. _Avoid_: Local Record, Message History

**Local Lock**: Optional full-database encryption for the Telegram authorization and Local Records of one Account. Unlock material comes from a hidden passphrase or an operating-system credential vault. It never enters application state, logs, or configuration. _Avoid_: Telegram 2FA, screen lock, Media Cache encryption

**Active Account**: The registered Telegram Account that the current process uses. Switching it updates `global.db`. Intuigram then opens only the authorization, navigation, Drafts, scroll position, records, cache, and notification identity for that Account. _Avoid_: Active Chat, shared session

**Clipboard Paste**: A context-sensitive Composer action that reads the platform clipboard directly. It inserts text into the Draft. It creates photo attachment candidates from images. It creates file attachment candidates from copied files. Unavailable or unsupported clipboard formats produce a visible failure and do not change the Draft. _Avoid_: Terminal text paste only, shell evaluation, path picker

**Logout**: A verified server-side revocation of the current Telegram authorization for an Account. Intuigram then deletes the local session, Local Records, and Media Cache for that Account. Logout succeeds only after Telegram acknowledges the revocation. _Avoid_: Remove locally, clear media cache, exit

**Remove Locally**: A destructive deletion of the local session, Local Records, and Media Cache for an Account. It does not state that Telegram revoked the server-side authorization. It gives a warning that the user can have to terminate the authorization from a different Telegram client. _Avoid_: Logout, clear media cache

**Rebuild Cache**: An explicit recovery operation. It replaces only redownloadable synchronized Account data. It preserves authorization, Drafts, Draft History, configuration, and other unique Local Records. If Intuigram cannot guarantee preservation, it must require export or explicit abandonment. It must not present the operation as a safe rebuild. _Avoid_: Delete Account, Logout, automatic database recreation

**Reconnect Cooldown**: A disconnected state. Automatic reconnection stops temporarily after unsuccessful attempts. Intuigram provides an explicit Reconnect action only in this state. _Avoid_: Any transient connection loss

**Pending Action**: An optimistic Telegram operation that Intuigram durably accepted but Telegram did not acknowledge. It stays visibly pending. Its Outbox item lets it continue through reconnection, process exit, and crashes. _Avoid_: Draft, completed action, transient request

**Outbox**: A durable FIFO for each Account that contains Pending Actions. Each item keeps versioned semantic intent, a stable Telegram operation identity, the exact referenced media, and explicit retry, cancellation, expiry, conflict, or unknown-outcome state. It stays there until acknowledgement or user resolution. _Avoid_: Draft, in-memory request queue

**Scheduled Message**: A Message that Telegram will deliver at a specified future time or, where supported, when the recipient is online. The scheduled-message history of a Chat manages it. It stays available after Intuigram exits. _Avoid_: Draft, Pending Action, Outbox item

**Current Action**: An operation that is available for the focused item in the active view. _Avoid_: Global shortcut

**Action Bar**: A persistent summary of all important Current Actions and their keys. Help contains complete and uncommon bindings. The Action Bar does not contain them. _Avoid_: Status bar, exhaustive key list

**Help**: The complete context-sensitive reference for bindings that are available from the active view. _Avoid_: Action Bar
