//! CDN-key-aware MTProto authorization state machine.
//!
//! The audited DH validation is kept equivalent to grammers-mtproto 0.10's
//! authentication module; only RSA trust-root selection is supplied by the
//! caller. The implementation remains here because grammers keeps its step
//! state private and accepts only Telegram's built-in master-DC key.

use grammers_mtproto::authentication::Error;
use num_bigint::BigUint;

mod dh;
mod request;

pub(super) use dh::{create_key, step3};
pub(super) use request::step2;

pub(super) struct Step2 {
    nonce: [u8; 16],
    server_nonce: [u8; 16],
    new_nonce: [u8; 32],
}

pub(super) struct Step3 {
    nonce: [u8; 16],
    server_nonce: [u8; 16],
    new_nonce: [u8; 32],
    gab: BigUint,
    time_offset: i32,
}

fn check_nonce(got: &[u8; 16], expected: &[u8; 16]) -> Result<(), Error> {
    if got == expected {
        Ok(())
    } else {
        Err(Error::InvalidNonce {
            got: *got,
            expected: *expected,
        })
    }
}

fn check_server_nonce(got: &[u8; 16], expected: &[u8; 16]) -> Result<(), Error> {
    if got == expected {
        Ok(())
    } else {
        Err(Error::InvalidServerNonce {
            got: *got,
            expected: *expected,
        })
    }
}

fn check_new_nonce_hash(got: &[u8; 16], expected: &[u8; 16]) -> Result<(), Error> {
    if got == expected {
        Ok(())
    } else {
        Err(Error::InvalidNewNonceHash {
            got: *got,
            expected: *expected,
        })
    }
}

fn check_g_in_range(value: &BigUint, low: &BigUint, high: &BigUint) -> Result<(), Error> {
    if low < value && value < high {
        Ok(())
    } else {
        Err(Error::GParameterOutOfRange {
            value: value.clone(),
            low: low.clone(),
            high: high.clone(),
        })
    }
}
