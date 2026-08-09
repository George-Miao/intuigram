mod codec;
mod endpoints;
mod lifecycle;
mod mapping;
mod repository;
mod types;

pub(in crate::account) use endpoints::{OutboxCommand, execute};
pub(in crate::account) use lifecycle::recover_in_flight;
pub use repository::Error;
pub(crate) use repository::{load, restore};
pub use types::{
    OutboxAdmission, OutboxExpiry, OutboxId, OutboxMedia, OutboxOperation, OutboxPayload,
    OutboxPayloadV1, OutboxRecord, OutboxState,
};
