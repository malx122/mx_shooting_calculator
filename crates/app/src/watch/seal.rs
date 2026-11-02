//! The watch link's encryption (docs/watch.md, "Encryption"): ECDH on P-256
//! between the watch's key and this app's, then AES-256-CBC and
//! HMAC-SHA256, the most a Connect IQ watch can check and decrypt.

use aes::Aes256;
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use hmac::{Hmac, KeyInit, Mac};
use p256::elliptic_curve::sec1::{FromSec1Point, ToSec1Point};
use p256::{PublicKey, Sec1Point, SecretKey};
use sha2::{Digest, Sha256};

/// A public key on the wire: X then Y, 32 bytes each, big-endian.
pub type WireKey = [u8; 64];

const IV_LEN: usize = 16;
const TAG_LEN: usize = 32;
/// The largest box the server takes (decoded).
pub const MAX_BOX: usize = 12 * 1024;

/// Fresh random bytes, from the same source as the app's ids (the
/// system's, or the browser's, random numbers).
pub fn random32() -> [u8; 32] {
    Sha256::new()
        .chain_update(ballistics_store::model::new_id())
        .chain_update(ballistics_store::model::new_id())
        .chain_update(ballistics_store::model::new_id())
        .finalize()
        .into()
}

/// A new key pair.
pub fn new_secret() -> SecretKey {
    loop {
        // Almost every 32 bytes are a valid key; the rest are tried again.
        if let Ok(secret) = SecretKey::from_bytes(&random32().into()) {
            return secret;
        }
    }
}

pub fn wire_key(public: &PublicKey) -> WireKey {
    let point = public.to_sec1_point(false);
    let mut key = [0; 64];
    // Uncompressed: 0x04, X, Y.
    key.copy_from_slice(&point.as_bytes()[1..65]);
    key
}

pub fn public_key(wire: &[u8]) -> Option<PublicKey> {
    if wire.len() != 64 {
        return None;
    }
    let point = Sec1Point::from_untagged_bytes(wire.try_into().ok()?);
    PublicKey::from_sec1_point(&point).into_option()
}

struct Keys {
    enc: [u8; 32],
    mac: [u8; 32],
}

fn keys(secret: &SecretKey, other: &PublicKey) -> Keys {
    let shared = p256::ecdh::diffie_hellman(secret.to_nonzero_scalar(), other.as_affine());
    let derive = |label: &[u8]| -> [u8; 32] {
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(shared.raw_secret_bytes())
            .expect("HMAC takes any key length");
        mac.update(label);
        mac.finalize().into_bytes().into()
    };
    Keys {
        enc: derive(b"mxsc watch v1 enc"),
        mac: derive(b"mxsc watch v1 mac"),
    }
}

fn tag(keys: &Keys, watch: &WireKey, app: &WireKey, iv: &[u8], ciphertext: &[u8]) -> [u8; 32] {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&keys.mac).expect("any length");
    mac.update(watch);
    mac.update(app);
    mac.update(iv);
    mac.update(ciphertext);
    mac.finalize().into_bytes().into()
}

/// Encrypts `table` for the watch whose public key is `watch`, with the
/// app's key `secret` and `iv`: the box to send.
pub fn seal_with(secret: &SecretKey, iv: [u8; IV_LEN], watch: &PublicKey, table: &[u8]) -> Vec<u8> {
    let keys = keys(secret, watch);
    let ciphertext = cbc::Encryptor::<Aes256>::new(&keys.enc.into(), &iv.into())
        .encrypt_padded_vec::<Pkcs7>(table);
    let tag = tag(
        &keys,
        &wire_key(watch),
        &wire_key(&secret.public_key()),
        &iv,
        &ciphertext,
    );
    [iv.as_slice(), &ciphertext, &tag].concat()
}

/// Encrypts `plain` for the watch with the app's key and a fresh IV.
pub fn seal(secret: &SecretKey, watch: &PublicKey, plain: &[u8]) -> Vec<u8> {
    let mut iv = [0; IV_LEN];
    iv.copy_from_slice(&random32()[..IV_LEN]);
    seal_with(secret, iv, watch, plain)
}

/// What the watch does: checks and decrypts a box with its own key. Here
/// for the tests and the watch's test vectors.
#[cfg_attr(not(test), allow(dead_code))]
pub fn open(secret: &SecretKey, app: &PublicKey, sealed: &[u8]) -> Option<Vec<u8>> {
    if sealed.len() < IV_LEN + IV_LEN + TAG_LEN
        || !(sealed.len() - IV_LEN - TAG_LEN).is_multiple_of(16)
    {
        return None;
    }
    let (iv, rest) = sealed.split_at(IV_LEN);
    let (ciphertext, received) = rest.split_at(rest.len() - TAG_LEN);
    let keys = keys(secret, app);
    let expected = tag(
        &keys,
        &wire_key(&secret.public_key()),
        &wire_key(app),
        iv,
        ciphertext,
    );
    // Not secret-dependent timing that matters here, but cheap to avoid.
    if expected
        .iter()
        .zip(received)
        .fold(0, |difference, (a, b)| difference | (a ^ b))
        != 0
    {
        return None;
    }
    let iv: [u8; IV_LEN] = iv.try_into().ok()?;
    cbc::Decryptor::<Aes256>::new(&keys.enc.into(), &iv.into())
        .decrypt_padded_vec::<Pkcs7>(ciphertext)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_watch_opens_what_the_app_seals() {
        let watch = new_secret();
        let secret = new_secret();
        let table = b"format 2 and some rows".to_vec();
        let sealed = seal(&secret, &watch.public_key(), &table);
        let app = secret.public_key();
        // A fresh IV each time.
        assert_ne!(seal(&secret, &watch.public_key(), &table), sealed);
        assert_eq!(
            open(&watch, &app, &sealed).as_deref(),
            Some(table.as_slice())
        );
        // Nobody else can, and a changed byte is refused.
        assert_eq!(open(&new_secret(), &app, &sealed), None);
        for at in [0, 20, sealed.len() - 1] {
            let mut changed = sealed.clone();
            changed[at] ^= 1;
            assert_eq!(open(&watch, &app, &changed), None, "byte {at}");
        }
        assert_eq!(open(&watch, &app, &sealed[..sealed.len() - 16]), None);
    }

    #[test]
    fn keys_go_over_the_wire_and_back() {
        let public = new_secret().public_key();
        let wire = wire_key(&public);
        assert_eq!(public_key(&wire), Some(public));
        assert_eq!(public_key(&[1; 64]), None);
        assert_eq!(public_key(&wire[..63]), None);
    }
}
