//! Telegram client operations grouped behind the live-client interface.

use super::*;

mod bootstrap;
mod connection;
mod dialogs;
mod download;
mod folders;
mod history;
mod initialize;
mod links;
mod location;
mod metadata;
mod notifications;
mod phone;
mod poll;
mod qr;
mod rich_media;
mod saved_dialogs;
mod scheduled;
mod send;
mod session;
mod specialized;
mod topics;
mod vote;

use dialogs::DialogBatch;
pub use folders::FolderRules;
pub use location::{StaticLocationSend, VenueSend};
pub use poll::PollSend;
pub use rich_media::{ContactCardSend, LibraryMediaSend, MediaLibraryEntry, MediaLibraryKind};
pub use scheduled::{ScheduledDelivery, ScheduledMessage};
pub use send::{TextSend, UploadSend};
