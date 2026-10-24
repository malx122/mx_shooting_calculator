//! Sync between the hunter's devices through a shared copy somewhere else
//! (a small server, or a file in the hunter's cloud storage).
//!
//! With the sync server, the devices share a secret **sync key**. The copy
//! is the usual export (`PersonalData`, with deletions), encrypted with a
//! key derived from the sync key, so whoever stores it can't read it. It is
//! stored under an id that is also derived from the sync key (a hash), so
//! the server never learns the key itself. In the hunter's own Google Drive
//! the copy is the plain export, in the app's hidden folder.
//!
//! One round of sync: read the shared copy, merge it into this device
//! (`Store::sync_round`), and write the result back if the shared copy
//! lacks anything; the write must say which version of the copy it was
//! based on, and starts over if another device wrote in between. Nothing
//! here does any networking: the app moves the bytes.

use std::fmt;

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use sha2::{Digest, Sha256};

use crate::StoreError;
use crate::db::Store;
use crate::transfer::{ImportReport, PersonalData, read_export};

/// Marks an encrypted copy and its format: "MXS" and version 1.
const MAGIC: &[u8; 4] = b"MXS1";
const NONCE_LEN: usize = 24;
/// Crockford's base32: no I, L, O or U, so a key read aloud or typed
/// survives (I and L read as 1, O as 0).
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// The secret the devices share: 128 random bits.
#[derive(Clone, PartialEq, Eq)]
pub struct SyncKey([u8; 16]);

impl SyncKey {
    /// A new random key.
    pub fn generate() -> SyncKey {
        // Two random UUIDs (244 random bits), hashed down to 128 bits.
        let mut hash = Sha256::new();
        hash.update(uuid::Uuid::new_v4().as_bytes());
        hash.update(uuid::Uuid::new_v4().as_bytes());
        let digest = hash.finalize();
        let mut key = [0; 16];
        key.copy_from_slice(&digest[..16]);
        SyncKey(key)
    }

    /// Reads a key as shown by `Display`, in any case, with or without its
    /// dashes and spaces, or a link holding one (`…#sync=KEY`).
    pub fn parse(text: &str) -> Option<SyncKey> {
        let text = text.rsplit("sync=").next().unwrap_or(text);
        let mut bits: u32 = 0;
        let mut count = 0;
        let mut bytes = Vec::with_capacity(16);
        for character in text.chars() {
            let value = match character.to_ascii_uppercase() {
                '-' | ' ' => continue,
                'O' => 0,
                'I' | 'L' => 1,
                other => ALPHABET.iter().position(|&a| a as char == other)? as u32,
            };
            bits = (bits << 5) | value;
            count += 5;
            if count >= 8 {
                count -= 8;
                bytes.push((bits >> count) as u8);
                bits &= (1 << count) - 1;
            }
        }
        // 26 characters: 128 bits and two spare ones, which must be zero.
        (bytes.len() == 16 && bits == 0).then(|| SyncKey(bytes.try_into().expect("16 bytes")))
    }

    /// The key without dashes, for links.
    pub fn compact(&self) -> String {
        self.to_string().replace('-', "")
    }

    /// Where the shared copy is kept: a hash of the key, so the place
    /// doesn't give the key away.
    pub fn blob_id(&self) -> String {
        Sha256::new()
            .chain_update(b"mxsc-sync/id")
            .chain_update(self.0)
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        let key = Sha256::new()
            .chain_update(b"mxsc-sync/key")
            .chain_update(self.0)
            .finalize();
        XChaCha20Poly1305::new(&key)
    }

    /// Encrypts a copy for storing.
    pub fn seal(&self, plaintext: &[u8]) -> Vec<u8> {
        let mut nonce = [0; NONCE_LEN];
        nonce[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        nonce[16..].copy_from_slice(&uuid::Uuid::new_v4().as_bytes()[..8]);
        let ciphertext = self
            .cipher()
            .encrypt(&XNonce::from(nonce), plaintext)
            .expect("encrypting in memory cannot fail");
        [MAGIC.as_slice(), &nonce, &ciphertext].concat()
    }

    /// Decrypts a stored copy; fails if it was made with another key or
    /// changed on the way.
    pub fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, StoreError> {
        let wrong = || StoreError::Invalid("the synced copy can't be read with this key".into());
        let rest = sealed.strip_prefix(MAGIC.as_slice()).ok_or_else(wrong)?;
        if rest.len() < NONCE_LEN {
            return Err(wrong());
        }
        let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
        let nonce = XNonce::try_from(nonce).map_err(|_| wrong())?;
        self.cipher()
            .decrypt(&nonce, ciphertext)
            .map_err(|_| wrong())
    }
}

impl fmt::Display for SyncKey {
    /// 26 characters in groups of four: `7K3M-QX2P-…`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut text = String::with_capacity(32);
        let mut bits: u32 = 0;
        let mut count = 0;
        let push = |value: u32, text: &mut String| {
            if !text.is_empty() && text.len() % 5 == 4 {
                text.push('-');
            }
            text.push(ALPHABET[value as usize] as char);
        };
        for byte in self.0 {
            bits = (bits << 8) | byte as u32;
            count += 8;
            while count >= 5 {
                count -= 5;
                push((bits >> count) & 31, &mut text);
            }
            bits &= (1 << count) - 1;
        }
        if count > 0 {
            push((bits << (5 - count)) & 31, &mut text);
        }
        formatter.write_str(&text)
    }
}

impl fmt::Debug for SyncKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Keep the secret out of logs.
        formatter.write_str("SyncKey(…)")
    }
}

/// What one round of sync did, and what to send back.
#[derive(Debug)]
pub struct SyncRound {
    /// What merging the shared copy changed here.
    pub report: ImportReport,
    /// The merged copy, encrypted, when the shared copy lacks anything.
    pub upload: Option<Vec<u8>>,
}

impl Store {
    /// Merges the shared copy (`None` when there is none yet) into this
    /// device, and says what to write back. With a key the copy is
    /// encrypted; without, it is plain JSON.
    pub fn sync_round(
        &mut self,
        key: Option<&SyncKey>,
        shared: Option<&[u8]>,
    ) -> Result<SyncRound, StoreError> {
        let theirs = match shared {
            Some(bytes) => {
                let json = match key {
                    Some(key) => key.open(bytes)?,
                    None => bytes.to_vec(),
                };
                let text = String::from_utf8(json)
                    .map_err(|_| StoreError::Invalid("the synced copy is damaged".into()))?;
                Some(read_export(&text)?)
            }
            None => None,
        };
        let report = match &theirs {
            Some(data) => self.merge_sync(data)?,
            None => ImportReport::default(),
        };
        let ours = self.export_data()?;
        let upload = match &theirs {
            Some(data) if same_records(data, &ours) => None,
            _ => {
                let json = serde_json::to_vec(&ours)?;
                Some(match key {
                    Some(key) => key.seal(&json),
                    None => json,
                })
            }
        };
        Ok(SyncRound { report, upload })
    }
}

/// Whether two copies hold the same records and deletions, in any order.
fn same_records(a: &PersonalData, b: &PersonalData) -> bool {
    fn canonical(data: &PersonalData) -> Option<serde_json::Value> {
        let mut value = serde_json::to_value(data).ok()?;
        let fields = value.as_object_mut()?;
        fields.remove("exported_at");
        fields.remove("format");
        fields.remove("version");
        for list in fields.values_mut() {
            if let Some(items) = list.as_array_mut() {
                items.sort_by_key(|item| {
                    let field = |name: &str| item.get(name).and_then(|v| v.as_str()).unwrap_or("");
                    (field("kind").to_owned(), field("id").to_owned())
                });
            }
        }
        Some(value)
    }
    canonical(a).is_some() && canonical(a) == canonical(b)
}
