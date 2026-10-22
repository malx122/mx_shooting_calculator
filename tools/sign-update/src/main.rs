//! `sign-update <file>`: writes `<file>.minisig`, a minisign signature of
//! the file made with the secret key in `MXSC_UPDATE_KEY` (the key file's
//! text, unencrypted). The desktop app checks it with the public key built
//! into it before it installs an update.

use std::io::Cursor;

use ct_codecs::{Base64, Decoder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: sign-update <file>")?;
    let key = std::env::var("MXSC_UPDATE_KEY").map_err(|_| "MXSC_UPDATE_KEY is not set")?;
    // The key file: a comment line, then the unencrypted key in base64.
    let encoded = key
        .lines()
        .nth(1)
        .ok_or("MXSC_UPDATE_KEY is not a key file")?;
    let mut buffer = vec![0u8; encoded.len()];
    let secret = minisign::SecretKey::from_bytes(Base64::decode(&mut buffer, encoded, None)?)?;
    let data = std::fs::read(&path)?;
    let name = std::path::Path::new(&path)
        .file_name()
        .map_or(path.clone(), |name| name.to_string_lossy().into_owned());
    let comment = format!("MX Shooting Calculator update: {name}");
    let signature = minisign::sign(None, &secret, Cursor::new(&data), Some(&comment), None)?;
    std::fs::write(format!("{path}.minisig"), signature.to_string())?;
    Ok(())
}
