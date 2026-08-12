use compio::buf::{IoBuf, IoBufExt as _};
use compio::io::framed::frame::Framer;

use super::{AbridgedCodec, AbridgedFramer};

#[test]
fn compio_framer_encodes_short_and_long_frames_with_one_preamble() {
    let mut codec = AbridgedCodec::new();
    let mut framer = AbridgedFramer::new();
    let mut buffer = Vec::new();
    codec
        .encode(&[0x42; 8], &mut buffer)
        .expect("short payload should encode");
    framer.enclose(&mut buffer);
    assert_eq!(&buffer[..2], &[0xef, 2]);
    assert_eq!(&buffer[2..], [0x42; 8]);

    codec
        .encode(&vec![0x24; 127 * 4], &mut buffer)
        .expect("long payload should encode");
    framer.enclose(&mut buffer);
    assert_eq!(&buffer[..4], &[0x7f, 0x7f, 0, 0]);
    assert_eq!(&buffer[4..], vec![0x24; 127 * 4]);
}

#[test]
fn compio_codec_reuses_the_supplied_buffer_allocation() {
    let mut codec = AbridgedCodec::new();
    let payload = vec![0x24; 512];
    let mut buffer = Vec::with_capacity(1024);
    codec
        .encode(&payload, &mut buffer)
        .expect("first payload should encode");
    let allocation = buffer.as_ptr();
    codec
        .encode(&payload, &mut buffer)
        .expect("second payload should encode");
    assert_eq!(buffer.as_ptr(), allocation);
}

#[test]
fn compio_framer_extracts_short_and_long_payloads() {
    let mut framer = AbridgedFramer::new();
    let short = vec![2, 1, 2, 3, 4, 5, 6, 7, 8];
    let short = short.slice(..);
    let frame = framer
        .extract(&short)
        .expect("short frame should be valid")
        .expect("short frame should be complete");
    assert_eq!(frame.slice(short).as_init(), &[1, 2, 3, 4, 5, 6, 7, 8]);

    let mut long = vec![0x7f, 0x80, 0, 0];
    long.extend(vec![0x55; 512]);
    let long = long.slice(..);
    let frame = framer
        .extract(&long)
        .expect("long frame should be valid")
        .expect("long frame should be complete");
    assert_eq!(frame.slice(long).as_init(), vec![0x55; 512]);
}

#[test]
fn unaligned_payloads_are_rejected() {
    assert!(
        AbridgedCodec::new()
            .encode(&[1, 2, 3], &mut Vec::new())
            .is_err()
    );
}

#[test]
fn encrypted_payload_prefix_is_not_misread_as_a_transport_status() {
    let mut payload = (-992_400_139_i32).to_le_bytes().to_vec();
    payload.extend_from_slice(&[1, 2, 3, 4]);

    assert_eq!(
        AbridgedCodec::decode_payload(&payload)
            .expect("an encrypted frame longer than four bytes is not a status"),
        payload
    );
}

#[test]
fn four_byte_negative_transport_status_is_rejected() {
    assert!(AbridgedCodec::decode_payload(&(-404_i32).to_le_bytes()).is_err());
}
