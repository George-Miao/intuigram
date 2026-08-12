use std::process::{Command, Output};

use tempfile::tempdir;

fn intuigram(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_intuigram"))
        .args(arguments)
        .env("CLICOLOR_FORCE", "1")
        .output()
        .expect("Intuigram should run")
}

#[test]
fn help_root_lists_command_groups() {
    let help = intuigram(&["--help"]);
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).expect("help should be UTF-8");
    for command in ["start", "account", "cache", "folder", "media", "scheduled"] {
        assert!(help.contains(command), "help should list {command:?}");
    }
}

#[test]
fn legacy_flag_parse_fails() {
    let output = intuigram(&["--media-cache-usage", "42"]);
    assert!(!output.status.success());
}

#[test]
fn global_options_nested_command_parse() {
    let help = intuigram(&[
        "cache",
        "usage",
        "--account",
        "42",
        "--data-dir",
        "/tmp/intuigram-data",
        "--help",
    ]);
    assert!(help.status.success());
}

#[test]
fn help_forced_color_emits_ansi() {
    let help = intuigram(&["--help"]);
    assert!(help.status.success());
    assert!(help.stdout.windows(2).any(|bytes| bytes == b"\x1b["));
}

#[test]
fn account_list_empty_store_succeeds() {
    let root = tempdir().expect("temporary root should open");
    let config = root.path().join("config");
    let data = root.path().join("data");
    let cache = root.path().join("cache");
    let downloads = root.path().join("downloads");
    let output = intuigram(&[
        "--config-dir",
        config.to_str().expect("temporary path should be UTF-8"),
        "--data-dir",
        data.to_str().expect("temporary path should be UTF-8"),
        "--cache-dir",
        cache.to_str().expect("temporary path should be UTF-8"),
        "--downloads-dir",
        downloads.to_str().expect("temporary path should be UTF-8"),
        "account",
        "list",
    ]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}
