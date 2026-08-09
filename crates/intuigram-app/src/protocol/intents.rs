use crate::domain::{ActivationTarget, ComposerMovement, ScrollDirection, ScrollTarget};

/// Context-sensitive actions shown by every user interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// Exit Intuigram cleanly.
    Quit,
    /// Open exhaustive context help.
    Help,
    /// Move the active item upward.
    MoveUp,
    /// Move the active item downward.
    MoveDown,
    /// Switch to the previous Folder from the Chat list.
    PreviousFolder,
    /// Switch to the next Folder from the Chat list.
    NextFolder,
    /// Open Folder membership for the Active Chat.
    ManageFolders,
    /// Open custom Folder lifecycle management.
    ManageFolderLifecycle,
    /// Start creating a custom Folder.
    CreateFolder,
    /// Edit the selected custom Folder.
    EditFolder,
    /// Save the active Folder editor.
    SaveFolder,
    /// Move the selected custom Folder earlier.
    ReorderFolderUp,
    /// Move the selected custom Folder later.
    ReorderFolderDown,
    /// Export a share link for the selected Folder.
    ShareFolder,
    /// Ask to delete the selected custom Folder.
    DeleteFolder,
    /// Confirm deletion of the selected custom Folder.
    ConfirmDeleteFolder,
    /// Toggle the selected Folder rule.
    ToggleFolderRule,
    /// Open the Account picker from Chat-list navigation.
    ManageAccounts,
    /// Confirm the selected Account or Add Account entry.
    ConfirmAccount,
    /// Ask to revoke and remove the selected active Account.
    LogoutAccount,
    /// Ask to remove the selected Account's local data only.
    RemoveAccountLocally,
    /// Confirm the pending destructive Account operation.
    ConfirmAccountOperation,
    /// Toggle the selected Folder membership for the Active Chat.
    ToggleFolderMembership,
    /// Enter the Active Chat with its Composer focused.
    Open,
    /// Open the context actions grouped for the current interaction target.
    OpenActions,
    /// Invoke the selected action in the context-actions popup.
    ChooseAction,
    /// Focus the Draft editor.
    Compose,
    /// Send the current Draft.
    Send,
    /// Insert a line break into the current Draft.
    Newline,
    /// Query the native clipboard for text, images, or files.
    Paste,
    /// Open the built-in attachment path editor.
    Attach,
    /// Add the exact path entered in the attachment editor.
    ConfirmAttachment,
    /// Open rich-media choices from the active Composer.
    OpenRichMedia,
    /// Activate the selected rich-media choice or submit its editor.
    ChooseRichMedia,
    /// Cycle the upload kind in the local-file editor.
    CycleRichMediaKind,
    /// Open server-owned Scheduled Message history for the Active Chat.
    OpenScheduled,
    /// Begin a new Scheduled Message.
    NewScheduled,
    /// Edit the selected Scheduled Message text.
    EditScheduled,
    /// Change the selected Scheduled Message delivery trigger.
    RescheduleScheduled,
    /// Request deletion of the selected Scheduled Message.
    DeleteScheduled,
    /// Request immediate delivery of the selected Scheduled Message.
    SendScheduledNow,
    /// Save the active Scheduled Message form.
    SaveScheduled,
    /// Confirm a Scheduled Message delete or immediate-send operation.
    ConfirmScheduled,
    /// Replace the Composer with a structured poll editor.
    CreatePoll,
    /// Send the question and options from the poll editor.
    SendPoll,
    /// Reply to the Active Message.
    Reply,
    /// Edit the Active outgoing Message.
    Edit,
    /// Edit the newest eligible outgoing Message from an empty Composer.
    EditPrevious,
    /// Ask for confirmation before deleting the Active Message.
    Delete,
    /// Confirm the pending Message deletion.
    ConfirmDelete,
    /// Choose a destination Chat for the Active Message.
    Forward,
    /// Confirm the selected forward destination.
    ConfirmForward,
    /// Open reactions for the Active Message.
    React,
    /// Apply the selected reaction.
    ConfirmReaction,
    /// Open voting for the Active Message's poll or quiz.
    VotePoll,
    /// Toggle the targeted option in a multiple-choice poll.
    TogglePollChoice,
    /// Submit the selected poll options.
    ConfirmPollVote,
    /// Open the first link in the Active Message.
    OpenLink,
    /// Confirm a suspicious or disguised link destination.
    ConfirmOpenLink,
    /// Download the Active Message's media to the default destination.
    DownloadMedia,
    /// Choose an exact destination for the Active Message's media.
    SaveAs,
    /// Download media to the entered exact destination.
    ConfirmSaveAs,
    /// Open a safe download or reveal launchable content in its folder.
    OpenDownload,
    /// Save the Message currently open for editing.
    SaveEdit,
    /// Open the Active Message's ordinary Thread or Channel comments.
    OpenThread,
    /// Target the newest pinned Message, then cycle toward older pins.
    NavigatePinned,
    /// Pin or unpin the Active cloud Message.
    TogglePin,
    /// Add or remove the Active Message from Message Selection.
    ToggleMessageSelection,
    /// Target the previous Message, entering the Transcript from the Composer.
    TargetPreviousMessage,
    /// Target the next Message, returning to the Composer after the newest.
    TargetNextMessage,
    /// Search using the context selected by focus.
    Search,
    /// Cancel the active transient interaction.
    Cancel,
    /// Jump to the oldest loaded Message.
    JumpEarliest,
    /// Jump to the newest loaded Message.
    JumpLatest,
    /// Retry immediately during a reconnect cooldown.
    Reconnect,
}

/// User actions understood by the state owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Intent {
    /// Invoke an action resolved by the effective keymap.
    Action(Action),
    /// Insert text into the Draft or active search query.
    Insert(String),
    /// Remove the final character from the active text field.
    Backspace,
    /// Move the insertion cursor without changing the Draft.
    MoveComposerCursor(ComposerMovement),
    /// Focus the Composer and place its cursor at a UTF-8 byte offset.
    SetComposerCursor(usize),
    /// Activate a semantic region selected by pointer input.
    Activate(ActivationTarget),
    /// Scroll the application region under the pointer.
    Scroll(ScrollTarget, ScrollDirection),
    /// Advance one renderer animation frame while pending work remains.
    Animate,
}
