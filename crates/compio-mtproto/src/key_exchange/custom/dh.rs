// Adapted from grammers-mtproto 0.10's MIT/Apache-2.0 authorization state
// machine so CDN-specific RSA roots can retain the same DH validation.

use std::time::{SystemTime, UNIX_EPOCH};

use grammers_crypto::AuthKey;
use grammers_mtproto::authentication::Error;
use grammers_tl_types::{self as tl, Cursor, Deserializable as _, Serializable as _};
use num_bigint::{BigUint, ToBigUint as _};
use sha1::{Digest as _, Sha1};

use super::{
    Step2, Step3, check_g_in_range, check_new_nonce_hash, check_nonce, check_server_nonce,
};

pub(in crate::key_exchange) struct Finished {
    pub(in crate::key_exchange) auth_key: [u8; 256],
    pub(in crate::key_exchange) time_offset: i32,
    pub(in crate::key_exchange) first_salt: i64,
}

pub(in crate::key_exchange) fn step3(
    data: Step2,
    response: tl::enums::ServerDhParams,
    random_bytes: &[u8; 256 + 16],
) -> Result<(tl::functions::SetClientDhParams, Step3), Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("MTProto key exchange requires a Unix-era wall clock")
        .as_secs() as i32;
    build_client_dh(data, response, random_bytes, now)
}

fn build_client_dh(
    data: Step2,
    response: tl::enums::ServerDhParams,
    random_bytes: &[u8; 256 + 16],
    now: i32,
) -> Result<(tl::functions::SetClientDhParams, Step3), Error> {
    let Step2 {
        nonce,
        server_nonce,
        new_nonce,
    } = data;
    let mut params = match response {
        tl::enums::ServerDhParams::Fail(params) => {
            check_nonce(&params.nonce, &nonce)?;
            check_server_nonce(&params.server_nonce, &server_nonce)?;
            let digest = Sha1::digest(new_nonce);
            let expected = digest[4..20]
                .try_into()
                .expect("SHA-1 contains the required nonce hash");
            check_new_nonce_hash(&params.new_nonce_hash, &expected)?;
            return Err(Error::DhParamsFail);
        }
        tl::enums::ServerDhParams::Ok(params) => params,
    };
    check_nonce(&params.nonce, &nonce)?;
    check_server_nonce(&params.server_nonce, &server_nonce)?;
    if params.encrypted_answer.len() < 32 || params.encrypted_answer.len() % 16 != 0 {
        return Err(Error::EncryptedResponseNotPadded {
            len: params.encrypted_answer.len(),
        });
    }

    let (key, iv) = grammers_crypto::generate_key_data_from_nonce(&server_nonce, &new_nonce);
    grammers_crypto::aes::ige_decrypt(&mut params.encrypted_answer, &key, &iv);
    let answer = params.encrypted_answer;
    let got_hash = answer[..20]
        .try_into()
        .expect("the encrypted answer minimum was checked");
    let mut cursor = Cursor::from_slice(&answer[20..]);
    let inner = match tl::enums::ServerDhInnerData::deserialize(&mut cursor) {
        Ok(tl::enums::ServerDhInnerData::Data(inner)) => inner,
        Err(error) => return Err(Error::InvalidDhInnerData { error }),
    };
    let expected_hash: [u8; 20] = Sha1::digest(&answer[20..20 + cursor.pos()]).into();
    if got_hash != expected_hash {
        return Err(Error::InvalidAnswerHash {
            got: got_hash,
            expected: expected_hash,
        });
    }
    check_nonce(&inner.nonce, &nonce)?;
    check_server_nonce(&inner.server_nonce, &server_nonce)?;

    let dh_prime = BigUint::from_bytes_be(&inner.dh_prime);
    let g = inner
        .g
        .to_biguint()
        .expect("Telegram's DH generator must be positive");
    let g_a = BigUint::from_bytes_be(&inner.g_a);
    let time_offset = inner.server_time - now;
    let b = BigUint::from_bytes_be(&random_bytes[..256]);
    let g_b = g.modpow(&b, &dh_prime);
    let gab = g_a.modpow(&b, &dh_prime);
    validate_dh_parameters(&g, &g_a, &g_b, &dh_prime)?;

    let inner = tl::enums::ClientDhInnerData::Data(tl::types::ClientDhInnerData {
        nonce,
        server_nonce,
        retry_id: 0,
        g_b: g_b.to_bytes_be(),
    })
    .to_bytes();
    let digest = Sha1::digest(&inner);
    let mut hashed = Vec::with_capacity(20 + inner.len() + 16);
    hashed.extend_from_slice(&digest);
    hashed.extend_from_slice(&inner);
    let padding = (16 - (hashed.len() % 16)) % 16;
    hashed.extend_from_slice(&random_bytes[256..256 + padding]);
    grammers_crypto::aes::ige_encrypt(&mut hashed, &key, &iv);

    Ok((
        tl::functions::SetClientDhParams {
            nonce,
            server_nonce,
            encrypted_data: hashed,
        },
        Step3 {
            nonce,
            server_nonce,
            new_nonce,
            gab,
            time_offset,
        },
    ))
}

fn validate_dh_parameters(
    g: &BigUint,
    g_a: &BigUint,
    g_b: &BigUint,
    prime: &BigUint,
) -> Result<(), Error> {
    let one = BigUint::from(1_u8);
    check_g_in_range(g, &one, &(prime - &one))?;
    check_g_in_range(g_a, &one, &(prime - &one))?;
    check_g_in_range(g_b, &one, &(prime - &one))?;
    let safety = one << (2048 - 64);
    check_g_in_range(g_a, &safety, &(prime - &safety))?;
    check_g_in_range(g_b, &safety, &(prime - &safety))
}

pub(in crate::key_exchange) fn create_key(
    data: Step3,
    response: tl::enums::SetClientDhParamsAnswer,
) -> Result<Finished, Error> {
    let Step3 {
        nonce,
        server_nonce,
        new_nonce,
        gab,
        time_offset,
    } = data;
    let (got_nonce, got_server_nonce, got_hash, nonce_number) = match response {
        tl::enums::SetClientDhParamsAnswer::DhGenOk(answer) => {
            (answer.nonce, answer.server_nonce, answer.new_nonce_hash1, 1)
        }
        tl::enums::SetClientDhParamsAnswer::DhGenRetry(answer) => {
            (answer.nonce, answer.server_nonce, answer.new_nonce_hash2, 2)
        }
        tl::enums::SetClientDhParamsAnswer::DhGenFail(answer) => {
            (answer.nonce, answer.server_nonce, answer.new_nonce_hash3, 3)
        }
    };
    check_nonce(&got_nonce, &nonce)?;
    check_server_nonce(&got_server_nonce, &server_nonce)?;
    let mut bytes = [0; 256];
    let gab = gab.to_bytes_be();
    let offset = bytes.len() - gab.len();
    bytes[offset..].copy_from_slice(&gab);
    let auth_key = AuthKey::from_bytes(bytes);
    let expected_hash = auth_key.calc_new_nonce_hash(&new_nonce, nonce_number);
    check_new_nonce_hash(&got_hash, &expected_hash)?;
    if nonce_number != 1 {
        return Err(if nonce_number == 2 {
            Error::DhGenRetry
        } else {
            Error::DhGenFail
        });
    }
    let mut salt = [0; 8];
    for (result, (client, server)) in salt
        .iter_mut()
        .zip(new_nonce.iter().zip(server_nonce.iter()))
    {
        *result = client ^ server;
    }
    Ok(Finished {
        auth_key: auth_key.to_bytes(),
        time_offset,
        first_salt: i64::from_le_bytes(salt),
    })
}
