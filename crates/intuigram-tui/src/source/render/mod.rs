//! TUI rendering grouped behind the source-level rendering interface.

use super::*;

pub(in crate::source) mod accounts;
pub(in crate::source) mod chrome;
pub(in crate::source) mod composer;
pub(in crate::source) mod details;
pub(in crate::source) mod folder_manager;
pub(in crate::source) mod headers;
pub(crate) mod layout;
pub(in crate::source) mod outbox;
pub(in crate::source) mod overlays;
pub(in crate::source) mod rich_media;
pub(in crate::source) mod saved_dialogs;
pub(in crate::source) mod scheduled;
pub(crate) mod text;
pub(in crate::source) mod topics;
pub(in crate::source) mod transcript;
