use std::time::{SystemTime, UNIX_EPOCH};

use snafu::ResultExt;

use super::error::{SystemClockBeforeEpochSnafu, SystemClockOverflowSnafu};
use super::{Clock, Result};

/// Production wall clock backed by the operating system.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn unix_seconds(&self) -> Result<i64> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context(SystemClockBeforeEpochSnafu)?;
        i64::try_from(elapsed.as_secs()).context(SystemClockOverflowSnafu)
    }
}
