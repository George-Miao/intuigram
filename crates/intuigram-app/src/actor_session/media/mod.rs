//! Actor-owned media dispatch and response normalization.

use super::*;

mod dispatcher;
mod response;

pub(super) use dispatcher::{MediaDispatcher, MediaOperation};
