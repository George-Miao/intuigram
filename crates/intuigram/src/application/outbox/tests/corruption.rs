use super::fixtures::send_commands;
use crate::application::outbox::codec::{Error, decode, encode};

#[test]
fn short_header_is_reported_as_truncated() {
    assert!(matches!(decode(b"ICM"), Err(Error::Truncated)));
}

#[test]
fn wrong_magic_is_rejected() {
    let mut encoded = valid_encoding();
    encoded[0] = b'X';

    assert!(matches!(decode(&encoded), Err(Error::InvalidHeader)));
}

#[test]
fn unknown_version_is_rejected_before_content() {
    let mut encoded = valid_encoding();
    encoded[4] = 99;

    assert!(matches!(
        decode(&encoded),
        Err(Error::UnsupportedVersion { version: 99 })
    ));
}

#[test]
fn truncated_content_is_reported_as_corrupt() {
    let mut encoded = valid_encoding();
    encoded.pop();

    assert!(matches!(decode(&encoded), Err(Error::Corrupt { .. })));
}

#[test]
fn unknown_command_family_is_reported_as_corrupt() {
    let encoded = valid_encoding();
    let json = str::from_utf8(&encoded[5..]).expect("command body should be JSON");
    let json = json.replacen("\"kind\":\"text\"", "\"kind\":\"future\"", 1);
    let mut corrupt = encoded[..5].to_vec();
    corrupt.extend_from_slice(json.as_bytes());

    assert!(matches!(decode(&corrupt), Err(Error::Corrupt { .. })));
}

#[test]
fn unknown_fields_are_reported_as_corrupt() {
    let encoded = valid_encoding();
    let mut json = str::from_utf8(&encoded[5..])
        .expect("command body should be JSON")
        .to_owned();
    assert_eq!(json.pop(), Some('}'));
    json.push_str(",\"unexpected\":true}");
    let mut corrupt = encoded[..5].to_vec();
    corrupt.extend_from_slice(json.as_bytes());

    assert!(matches!(decode(&corrupt), Err(Error::Corrupt { .. })));
}

#[test]
fn trailing_content_is_reported_as_corrupt() {
    let mut encoded = valid_encoding();
    encoded.extend_from_slice(b" discarded");

    assert!(matches!(decode(&encoded), Err(Error::Corrupt { .. })));
}

fn valid_encoding() -> Vec<u8> {
    encode(&send_commands()[0]).expect("fixture should encode")
}
