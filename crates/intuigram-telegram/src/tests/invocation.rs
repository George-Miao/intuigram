use std::time::Duration;

use compio_mtproto::InvocationError;

use crate::{Error, InvocationPolicy, RetryDisposition, flood_wait_delay};

#[test]
fn wait_policy_retries_flood_wait_after_the_server_delay() {
    let error = InvocationError::Rpc {
        code: 420,
        message: "FLOOD_WAIT_17".to_owned(),
    };

    assert_eq!(
        flood_wait_delay(InvocationPolicy::WaitForFlood, &error),
        Some(Duration::from_secs(17))
    );
}

#[test]
fn surface_policy_returns_flood_wait_without_delay() {
    let error = InvocationError::Rpc {
        code: 420,
        message: "FLOOD_WAIT_17".to_owned(),
    };

    assert_eq!(
        flood_wait_delay(InvocationPolicy::SurfaceFloodWait, &error),
        None
    );
}

#[test]
fn flood_wait_exposes_its_retry_delay() {
    let error = Error::Invoke {
        source: InvocationError::Rpc {
            code: 420,
            message: "FLOOD_WAIT_17".to_owned(),
        },
    };

    assert_eq!(
        error.retry_disposition(),
        RetryDisposition::RetryAfter(Duration::from_secs(17))
    );
}

#[test]
fn stopped_driver_retries_after_reconnecting() {
    let error = Error::Invoke {
        source: InvocationError::DriverStopped,
    };

    assert_eq!(
        error.retry_disposition(),
        RetryDisposition::RetryAfterReconnect
    );
}

#[test]
fn saturated_queue_retries_when_capacity_is_available() {
    let error = Error::Invoke {
        source: InvocationError::QueueFull { capacity: 8 },
    };

    assert_eq!(
        error.retry_disposition(),
        RetryDisposition::RetryWhenCapacityAvailable
    );
}

#[test]
fn ordinary_rpc_error_is_not_retried() {
    let error = Error::Invoke {
        source: InvocationError::Rpc {
            code: 400,
            message: "MESSAGE_ID_INVALID".to_owned(),
        },
    };

    assert_eq!(error.retry_disposition(), RetryDisposition::DoNotRetry);
}

#[test]
fn non_flood_rpc_with_numeric_suffix_is_not_retried() {
    let error = Error::Invoke {
        source: InvocationError::Rpc {
            code: 400,
            message: "MESSAGE_ID_INVALID_17".to_owned(),
        },
    };

    assert_eq!(error.retry_disposition(), RetryDisposition::DoNotRetry);
}

#[test]
fn locally_rejected_request_is_not_retried() {
    let error = Error::Invoke {
        source: InvocationError::RequestTooLarge,
    };

    assert_eq!(error.retry_disposition(), RetryDisposition::DoNotRetry);
}
