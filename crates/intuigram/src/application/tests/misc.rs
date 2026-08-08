use std::io;
use std::path::PathBuf;

use intuigram_telegram::{LoginCodeDelivery, LoginCodeDeliveryMethod};

use super::super::runtime::connection_failure_reason;
use super::super::{
    Error, PRIMARY_DC_ENDPOINT, error_lines, login_code_delivery_message,
    login_code_delivery_method_name, parse_arguments, seconds_until_at,
};

#[test]
fn bootstrap_uses_the_production_dc_2_endpoint() {
    assert_eq!(PRIMARY_DC_ENDPOINT.to_string(), "149.154.167.41:443");
}

#[test]
fn qr_expiry_uses_the_telegram_server_time_offset() {
    assert_eq!(seconds_until_at(1_030, 1_000, 10), 20);
    assert_eq!(seconds_until_at(1_030, 1_000, 40), 0);
}

#[test]
fn telegram_app_login_codes_name_the_actual_destination() {
    assert_eq!(
        login_code_delivery_message(&LoginCodeDelivery::TelegramApp { length: 5 }),
        "Telegram sent a 5-digit code to the Telegram app on another logged-in device."
    );
}

#[test]
fn login_code_fallback_names_sms_delivery() {
    assert_eq!(
        login_code_delivery_method_name(LoginCodeDeliveryMethod::Sms),
        "SMS delivery"
    );
}

#[test]
fn command_line_paths_are_parsed_and_the_obsolete_demo_flag_is_rejected() {
    let parsed = parse_arguments([
        "--data-dir".to_owned(),
        "/tmp/intuigram-data".to_owned(),
        "--cache-dir".to_owned(),
        "/tmp/intuigram-cache".to_owned(),
    ])
    .expect("valid command line should parse");
    assert_eq!(
        parsed.data.expect("data override should exist"),
        PathBuf::from("/tmp/intuigram-data")
    );
    assert_eq!(
        parsed.cache.expect("cache override should exist"),
        PathBuf::from("/tmp/intuigram-cache")
    );
    assert!(parse_arguments(["--demo".to_owned()]).is_err());
}

#[test]
fn errors_are_rendered_one_line_per_source_layer() {
    let error = Error::Runtime {
        source: io::Error::other("driver setup\nfailed"),
    };
    assert_eq!(
        error_lines(&error),
        [
            "failed to initialize the Compio runtime",
            "driver setup failed"
        ]
    );
}

#[test]
fn synchronization_gap_enters_the_reconnect_path() {
    let error = Error::CommitTelegramUpdate {
        source: intuigram::SyncError::UpdateGap {
            scope: "channel:-1000000000005".to_owned(),
        },
    };
    assert!(connection_failure_reason(&error).is_some());
}
