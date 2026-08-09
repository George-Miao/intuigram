use std::collections::HashSet;

use snafu::ResultExt;

use super::error::{GenerateOperationIdSnafu, OperationIdCollisionsSnafu};
use super::{OperationIdSource, Result};

const COLLISION_LIMIT: usize = 64;

/// Production operation-ID source backed by operating-system entropy.
#[derive(Debug, Default)]
pub struct SecureOperationIds {
    issued: HashSet<i64>,
}

impl OperationIdSource for SecureOperationIds {
    fn next_id(&mut self) -> Result<i64> {
        for _ in 0..COLLISION_LIMIT {
            let mut bytes = [0_u8; 8];
            getrandom::fill(&mut bytes).context(GenerateOperationIdSnafu)?;
            let candidate = i64::from_le_bytes(bytes);
            if candidate != 0 && self.issued.insert(candidate) {
                return Ok(candidate);
            }
        }
        OperationIdCollisionsSnafu.fail()
    }
}
