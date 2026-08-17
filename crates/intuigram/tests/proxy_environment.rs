use std::process::{Command, Output};

use tempfile::tempdir;

fn intuigram_with_proxy(variable: &str, value: &str, intuigram_proxy: Option<&str>) -> Output {
    let root = tempdir().expect("temporary root should open");
    let mut command = Command::new(env!("CARGO_BIN_EXE_intuigram"));
    command.args([
        "--config-dir",
        root.path()
            .join("config")
            .to_str()
            .expect("temporary config path should be UTF-8"),
        "--data-dir",
        root.path()
            .join("data")
            .to_str()
            .expect("temporary data path should be UTF-8"),
        "--test-connection",
    ]);
    for variable in [
        "all_proxy",
        "ALL_PROXY",
        "https_proxy",
        "HTTPS_PROXY",
        "http_proxy",
        "HTTP_PROXY",
    ] {
        command.env_remove(variable);
    }
    command
        .env_remove("INTUIGRAM_CONNECTION__PROXIES")
        .env_remove("INTUIGRAM_CONNECTION__DIRECT_FALLBACK");
    if let Some(proxy) = intuigram_proxy {
        command.env("INTUIGRAM_CONNECTION__PROXIES", proxy);
    }
    command
        .env("INTUIGRAM_TELEGRAM__API_ID", "1")
        .env("INTUIGRAM_TELEGRAM__API_HASH", "dummy")
        .env("INTUIGRAM_CONNECTION__TIMEOUT_SECONDS", "1")
        .env(variable, value)
        .output()
        .expect("Intuigram should run")
}

#[test]
fn generic_proxy_variables_route_telegram() {
    for (variable, value, route) in [
        ("all_proxy", "socks5h://127.0.0.1:9", "SOCKS5"),
        ("ALL_PROXY", "socks5h://127.0.0.1:9", "SOCKS5"),
        ("https_proxy", "http://127.0.0.1:9", "HTTP CONNECT"),
        ("HTTPS_PROXY", "http://127.0.0.1:9", "HTTP CONNECT"),
        ("http_proxy", "http://127.0.0.1:9", "HTTP CONNECT"),
        ("HTTP_PROXY", "http://127.0.0.1:9", "HTTP CONNECT"),
    ] {
        let output = intuigram_with_proxy(variable, value, None);
        let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");

        assert!(
            !output.status.success(),
            "{variable} unexpectedly succeeded"
        );
        assert!(
            stderr.contains("all 2 Telegram transport routes failed"),
            "{variable}: {stderr}"
        );
        assert!(
            stderr.contains(&format!("{route} 127.0.0.1:9 to 149.154.167.41:443")),
            "{variable}: {stderr}"
        );
        assert!(
            stderr.contains(&format!(
                "{route} 127.0.0.1:9 to [2001:67c:4e8:f002::a]:443"
            )),
            "{variable}: {stderr}"
        );
        assert!(
            !stderr.contains("direct 149.154.167.41:443")
                && !stderr.contains("direct [2001:67c:4e8:f002::a]:443"),
            "{variable}: {stderr}"
        );
    }
}

#[test]
fn https_proxy_credentials_stay_redacted() {
    let output = intuigram_with_proxy(
        "HTTPS_PROXY",
        "http://proxy-user:proxy-password@127.0.0.1:9",
        None,
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");

    assert!(!output.status.success());
    assert!(stderr.contains("HTTP CONNECT 127.0.0.1:9"));
    assert!(!stderr.contains("proxy-user"));
    assert!(!stderr.contains("proxy-password"));
}

#[test]
fn intuigram_proxy_variables_override_generic_proxy() {
    let output = intuigram_with_proxy(
        "ALL_PROXY",
        "socks5h://127.0.0.1:8",
        Some(r#"[{kind="http-connect",host="127.0.0.1",port=9}]"#),
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");

    assert!(!output.status.success());
    assert!(stderr.contains("HTTP CONNECT 127.0.0.1:9"));
    assert!(!stderr.contains("SOCKS5 127.0.0.1:8"));
}
