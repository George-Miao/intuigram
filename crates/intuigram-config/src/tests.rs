use std::fs;
use std::path::Path;

use tempfile::tempdir;

use super::*;

fn defaults(root: &Path) -> PlatformDefaults {
    PlatformDefaults {
        config: root.join("config"),
        data: root.join("default-data"),
        cache: root.join("default-cache"),
        downloads: root.join("default-downloads"),
    }
}

#[test]
fn json_overrides_yaml_and_toml_from_the_default_config_directory() {
    let temporary = tempdir().expect("temporary directory should be created");
    let platform = defaults(temporary.path());
    fs::create_dir_all(&platform.config).expect("config directory should be created");
    fs::write(
        platform.config.join("config.toml"),
        "[media]\ncache_bytes = 1024\n",
    )
    .expect("TOML config should be written");
    fs::write(
        platform.config.join("config.yaml"),
        "media:\n  cache_bytes: 2048\n",
    )
    .expect("YAML config should be written");
    fs::write(
        platform.config.join("config.json"),
        "{\"media\": {\"cache_bytes\": 4096}}",
    )
    .expect("JSON config should be written");
    let config = ConfigLoader::new(platform)
        .read_environment(false)
        .load()
        .expect("layered configuration should load");
    assert_eq!(config.media.cache_bytes, 4096);
}

#[test]
fn command_line_values_override_configuration_files() {
    let temporary = tempdir().expect("temporary directory should be created");
    let platform = defaults(temporary.path());
    fs::create_dir_all(&platform.config).expect("config directory should be created");
    fs::write(
        platform.config.join("config.toml"),
        "[paths]\ndata = 'from-file'\n[media]\ncache_bytes = 'invalid lower source'\n",
    )
    .expect("TOML config should be written");
    let command_line_data = temporary.path().join("from-command-line");
    let config = ConfigLoader::new(platform)
        .read_environment(false)
        .with_overrides(Overrides {
            data: Some(command_line_data.clone()),
            media_cache_bytes: Some(8192),
            ..Overrides::default()
        })
        .load()
        .expect("layered configuration should load");
    assert_eq!(config.paths.data, command_line_data);
    assert_eq!(config.media.cache_bytes, 8192);
}

#[test]
fn log_path_defaults_to_data_and_accepts_an_override() {
    let temporary = tempdir().expect("temporary directory should be created");
    let platform = defaults(temporary.path());
    fs::create_dir_all(&platform.config).expect("config directory should be created");
    let data = temporary.path().join("command-data");
    let default = ConfigLoader::new(platform.clone())
        .read_environment(false)
        .with_overrides(Overrides {
            data: Some(data.clone()),
            ..Overrides::default()
        })
        .load()
        .expect("default logging configuration should load");
    assert_eq!(default.log_path(), data.join("intuigram.log"));
    let configured = temporary.path().join("diagnostics/client.log");
    fs::write(
        platform.config.join("config.toml"),
        format!("[logging]\npath = {:?}\n", configured.display().to_string()),
    )
    .expect("logging configuration should be written");
    let config = ConfigLoader::new(platform)
        .read_environment(false)
        .load()
        .expect("configured log path should load");
    assert_eq!(config.log_path(), configured);
}

#[test]
fn external_path_picker_is_loaded_as_a_direct_program_and_arguments() {
    let temporary = tempdir().expect("temporary directory should be created");
    let platform = defaults(temporary.path());
    fs::create_dir_all(&platform.config).expect("config directory should be created");
    fs::write(
        platform.config.join("config.toml"),
        "[external.path_picker]\nprogram = 'zenity'\nargs = ['--file-selection']\n",
    )
    .expect("TOML config should be written");

    let config = ConfigLoader::new(platform)
        .read_environment(false)
        .load()
        .expect("path-picker configuration should load");
    let picker = config
        .external
        .path_picker
        .expect("configured path picker should be present");

    assert_eq!(picker.program, "zenity");
    assert_eq!(picker.args, ["--file-selection"]);
}

#[test]
fn telegram_and_proxy_secrets_are_redacted() {
    let temporary = tempdir().expect("temporary directory should be created");
    let platform = defaults(temporary.path());
    fs::create_dir_all(&platform.config).expect("config directory should be created");
    fs::write(
        platform.config.join("config.toml"),
        "[telegram]\napi_id = 42\napi_hash = 'api-secret'\n\n[connection]\ndirect_fallback = \
         false\n\n[[connection.proxies]]\nkind = 'mt-proxy'\nhost = 'proxy.example'\nport = \
         443\nsecret = '00112233445566778899aabbccddeeff'\n",
    )
    .expect("TOML config should be written");
    let config = ConfigLoader::new(platform)
        .read_environment(false)
        .load()
        .expect("proxy configuration should load");
    let debug = format!("{config:?}");
    assert!(!debug.contains("api-secret"));
    assert!(!debug.contains("001122"));
    assert!(!config.connection.direct_fallback);
}

#[test]
fn spacious_view_and_telegram_like_message_width_are_configurable() {
    let temporary = tempdir().expect("temporary directory should be created");
    let platform = defaults(temporary.path());
    fs::create_dir_all(&platform.config).expect("config directory should be created");
    let default = ConfigLoader::new(platform.clone())
        .read_environment(false)
        .load()
        .expect("default configuration should load");
    assert_eq!(default.view.mode, ViewMode::Default);
    assert_eq!(default.view.message_max_width.get(), 96);
    fs::write(
        platform.config.join("config.toml"),
        "[view]\nmode = 'compact'\nmessage_max_width = 72\n",
    )
    .expect("TOML config should be written");
    let compact = ConfigLoader::new(platform)
        .read_environment(false)
        .load()
        .expect("compact view configuration should load");
    assert_eq!(compact.view.mode, ViewMode::Compact);
    assert_eq!(compact.view.message_max_width.get(), 72);
}

#[test]
fn local_lock_is_opt_in_with_explicit_unlock_source() {
    let temporary = tempdir().expect("temporary directory should be created");
    let platform = defaults(temporary.path());
    fs::create_dir_all(&platform.config).expect("config directory should be created");
    let unlocked = ConfigLoader::new(platform.clone())
        .read_environment(false)
        .load()
        .expect("default configuration should load");
    assert!(!unlocked.local_lock.enabled);
    fs::write(
        platform.config.join("config.toml"),
        "[local_lock]\nenabled = true\nunlock = 'keyring'\n",
    )
    .expect("Local Lock config should be written");
    let locked = ConfigLoader::new(platform)
        .read_environment(false)
        .load()
        .expect("Local Lock configuration should load");
    assert!(locked.local_lock.enabled);
    assert_eq!(locked.local_lock.unlock, UnlockMethod::Keyring);
}
