use base64::Engine as _;
use grammers_crypto::rsa;
use num_bigint::BigUint;
use sha1::{Digest as _, Sha1};

pub(super) struct RsaKey {
    pub(super) fingerprint: i64,
    pub(super) encryption: rsa::Key,
}

impl RsaKey {
    pub(super) fn from_pem(pem: &str) -> Option<Self> {
        let pkcs1 = pem.contains("BEGIN RSA PUBLIC KEY");
        let encoded = pem
            .lines()
            .filter(|line| !line.starts_with("-----"))
            .collect::<String>();
        let der = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .ok()?;
        let payload = sequence(&der)?;
        let rsa = if pkcs1 {
            payload
        } else {
            subject_public_key(payload)?
        };
        let mut cursor = rsa;
        let modulus = integer(&mut cursor)?;
        let exponent = integer(&mut cursor)?;
        if !cursor.is_empty() {
            return None;
        }
        let n = BigUint::from_bytes_be(modulus);
        let e = BigUint::from_bytes_be(exponent);
        let fingerprint = fingerprint(modulus, exponent);
        let encryption = rsa::Key::new(&n.to_str_radix(10), &e.to_str_radix(10))?;
        Some(Self {
            fingerprint,
            encryption,
        })
    }
}

fn subject_public_key(mut der: &[u8]) -> Option<&[u8]> {
    const RSA_ENCRYPTION: &[u8] = &[
        0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00,
    ];
    let algorithm = tagged(&mut der, 0x30)?;
    if algorithm != RSA_ENCRYPTION {
        return None;
    }
    let bits = tagged(&mut der, 0x03)?;
    if !der.is_empty() || bits.first().copied()? != 0 {
        return None;
    }
    sequence(&bits[1..])
}

fn sequence(der: &[u8]) -> Option<&[u8]> {
    let mut cursor = der;
    let sequence = tagged(&mut cursor, 0x30)?;
    cursor.is_empty().then_some(sequence)
}

fn integer<'a>(der: &mut &'a [u8]) -> Option<&'a [u8]> {
    let integer = tagged(der, 0x02)?;
    let integer = integer.strip_prefix(&[0]).unwrap_or(integer);
    (!integer.is_empty()).then_some(integer)
}

fn tagged<'a>(der: &mut &'a [u8], expected: u8) -> Option<&'a [u8]> {
    let (&tag, rest) = der.split_first()?;
    if tag != expected {
        return None;
    }
    let (length, rest) = length(rest)?;
    let (value, remaining) = rest.split_at_checked(length)?;
    *der = remaining;
    Some(value)
}

fn length(der: &[u8]) -> Option<(usize, &[u8])> {
    let (&first, rest) = der.split_first()?;
    if first & 0x80 == 0 {
        return Some((usize::from(first), rest));
    }
    let count = usize::from(first & 0x7f);
    if count == 0 || count > std::mem::size_of::<usize>() {
        return None;
    }
    let (bytes, rest) = rest.split_at_checked(count)?;
    let value = bytes.iter().try_fold(0usize, |value, byte| {
        value.checked_shl(8)?.checked_add(usize::from(*byte))
    })?;
    Some((value, rest))
}

fn fingerprint(modulus: &[u8], exponent: &[u8]) -> i64 {
    let mut serialized = serialize_bytes(modulus);
    serialized.extend(serialize_bytes(exponent));
    let digest = Sha1::digest(serialized);
    i64::from_le_bytes(
        digest[digest.len() - 8..]
            .try_into()
            .expect("SHA-1 always ends in eight fingerprint bytes"),
    )
}

fn serialize_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(bytes.len() + 8);
    if bytes.len() < 254 {
        result.push(bytes.len() as u8);
    } else {
        result.push(254);
        let length = bytes.len().to_le_bytes();
        result.extend_from_slice(&length[..3]);
    }
    result.extend_from_slice(bytes);
    result.resize(result.len().next_multiple_of(4), 0);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pkcs1_and_calculates_telegram_fingerprint() {
        let key = RsaKey::from_pem(
            "-----BEGIN RSA PUBLIC KEY-----\nMAcCAgyhAgER\n-----END RSA PUBLIC KEY-----",
        )
        .expect("minimal PKCS#1 key should parse");

        assert_eq!(key.fingerprint, 9_028_996_554_662_447_379);
    }

    #[test]
    fn rejects_trailing_der_data() {
        assert!(
            RsaKey::from_pem(
                "-----BEGIN RSA PUBLIC KEY-----\nMAcCAgyhAgERAA==\n-----END RSA PUBLIC KEY-----"
            )
            .is_none()
        );
    }
}
