// Adapted from grammers-mtproto 0.10's MIT/Apache-2.0 authorization state
// machine; the only behavioral change is caller-supplied RSA root selection.

use grammers_crypto::{factorize, rsa};
use grammers_mtproto::authentication::Error;
use grammers_tl_types::{self as tl, Serializable as _};

use super::super::rsa_key::RsaKey;
use super::{Step2, check_nonce};

pub(in crate::key_exchange) fn step2(
    nonce: [u8; 16],
    response: tl::enums::ResPq,
    keys: &[RsaKey],
    random_bytes: &[u8; 32 + 224],
) -> Result<(tl::functions::ReqDhParams, Step2), Error> {
    let tl::enums::ResPq::Pq(res_pq) = response;
    check_nonce(&res_pq.nonce, &nonce)?;
    if res_pq.pq.len() != 8 {
        return Err(Error::InvalidPqSize {
            size: res_pq.pq.len(),
        });
    }

    let pq = u64::from_be_bytes(
        res_pq
            .pq
            .as_slice()
            .try_into()
            .expect("the PQ byte length was checked"),
    );
    let (p, q) = factorize(pq);
    let new_nonce = random_bytes[..32]
        .try_into()
        .expect("the source slice has the nonce length");
    let random_padding: [u8; 224] = random_bytes[32..]
        .try_into()
        .expect("the source slice has the RSA padding length");
    let p_bytes = minimal_integer(p);
    let q_bytes = minimal_integer(q);
    let pq_inner_data = tl::enums::PQInnerData::Data(tl::types::PQInnerData {
        pq: pq.to_be_bytes().to_vec(),
        p: p_bytes.clone(),
        q: q_bytes.clone(),
        nonce,
        server_nonce: res_pq.server_nonce,
        new_nonce,
    })
    .to_bytes();
    let key = res_pq
        .server_public_key_fingerprints
        .iter()
        .find_map(|fingerprint| keys.iter().find(|key| key.fingerprint == *fingerprint))
        .ok_or_else(|| Error::UnknownFingerprints {
            fingerprints: res_pq.server_public_key_fingerprints.clone(),
        })?;
    let ciphertext = rsa::encrypt_hashed(&pq_inner_data, &key.encryption, &random_padding);

    Ok((
        tl::functions::ReqDhParams {
            nonce,
            server_nonce: res_pq.server_nonce,
            p: p_bytes,
            q: q_bytes,
            public_key_fingerprint: key.fingerprint,
            encrypted_data: ciphertext,
        },
        Step2 {
            nonce,
            server_nonce: res_pq.server_nonce,
            new_nonce,
        },
    ))
}

fn minimal_integer(value: u64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let position = bytes.iter().position(|byte| *byte != 0).unwrap_or(7);
    bytes[position..].to_vec()
}
