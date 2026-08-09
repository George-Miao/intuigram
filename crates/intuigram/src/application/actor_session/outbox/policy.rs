use std::time::Duration;

use intuigram_store::{OutboxOperation, OutboxState};
use intuigram_telegram::RetryDisposition;

const CAPACITY_RETRY_SECONDS: i64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Transition {
    Defer(i64),
    Fail,
    Conflict,
    OutcomeUnknown,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Decision {
    pub(super) transition: Transition,
    pub(super) reconnect: bool,
}

pub(super) fn decide(
    operation: OutboxOperation,
    state: OutboxState,
    disposition: RetryDisposition,
    reached_telegram: bool,
    now: i64,
) -> Decision {
    if state == OutboxState::CancelRequested {
        return Decision {
            transition: if reached_telegram && disposition == RetryDisposition::RetryAfterReconnect
            {
                Transition::OutcomeUnknown
            } else {
                Transition::Cancel
            },
            reconnect: disposition == RetryDisposition::RetryAfterReconnect,
        };
    }
    if !reached_telegram {
        return Decision {
            transition: Transition::Fail,
            reconnect: false,
        };
    }
    match disposition {
        RetryDisposition::RetryAfter(delay) => Decision {
            transition: Transition::Defer(retry_at(now, delay)),
            reconnect: false,
        },
        RetryDisposition::RetryWhenCapacityAvailable => Decision {
            transition: Transition::Defer(now.saturating_add(CAPACITY_RETRY_SECONDS)),
            reconnect: false,
        },
        RetryDisposition::RetryAfterReconnect => Decision {
            transition: if operation == OutboxOperation::Mutation {
                Transition::OutcomeUnknown
            } else {
                Transition::Defer(now.saturating_add(CAPACITY_RETRY_SECONDS))
            },
            reconnect: true,
        },
        RetryDisposition::DoNotRetry => Decision {
            transition: if operation == OutboxOperation::Mutation {
                Transition::Conflict
            } else {
                Transition::Fail
            },
            reconnect: false,
        },
    }
}

fn retry_at(now: i64, delay: Duration) -> i64 {
    let seconds = match i64::try_from(delay.as_secs()) {
        Ok(seconds) => seconds.max(1),
        Err(_) => i64::MAX,
    };
    now.saturating_add(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_send_waits_for_reconnect_with_its_stable_identity() {
        let decision = decide(
            OutboxOperation::Send,
            OutboxState::InFlight,
            RetryDisposition::RetryAfterReconnect,
            true,
            41,
        );

        assert_eq!(decision.transition, Transition::Defer(42));
        assert!(decision.reconnect);
    }

    #[test]
    fn mutation_with_a_lost_connection_requires_outcome_resolution() {
        let decision = decide(
            OutboxOperation::Mutation,
            OutboxState::InFlight,
            RetryDisposition::RetryAfterReconnect,
            true,
            41,
        );

        assert_eq!(decision.transition, Transition::OutcomeUnknown);
        assert!(decision.reconnect);
    }

    #[test]
    fn cancelled_connection_failure_is_not_claimed_to_be_unsent() {
        let decision = decide(
            OutboxOperation::Send,
            OutboxState::CancelRequested,
            RetryDisposition::RetryAfterReconnect,
            true,
            41,
        );

        assert_eq!(decision.transition, Transition::OutcomeUnknown);
    }

    #[test]
    fn local_corruption_fails_without_replay() {
        let decision = decide(
            OutboxOperation::Send,
            OutboxState::InFlight,
            RetryDisposition::DoNotRetry,
            false,
            41,
        );

        assert_eq!(decision.transition, Transition::Fail);
        assert!(!decision.reconnect);
    }
}
