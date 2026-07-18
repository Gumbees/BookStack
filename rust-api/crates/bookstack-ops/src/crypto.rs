//! Backup encryption: XChaCha20-Poly1305 AEAD with an argon2id-derived key.
//! File layout: `BSBK1` magic || 16-byte salt || 24-byte nonce || ciphertext.

use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{AeadCore, XChaCha20Poly1305, XNonce};
use rand::RngCore;

use crate::{OpsError, Result};

const MAGIC: &[u8; 5] = b"BSBK1";

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| OpsError::Crypto(e.to_string()))?;
    Ok(key)
}

pub fn encrypt(passphrase: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    let key = derive_key(passphrase, &salt)?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| OpsError::Crypto(e.to_string()))?;

    let mut out = Vec::with_capacity(MAGIC.len() + 16 + 24 + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt(passphrase: &str, data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < MAGIC.len() + 16 + 24 || &data[..MAGIC.len()] != MAGIC {
        return Err(OpsError::Crypto("not a BookStack encrypted backup (bad header)".into()));
    }
    let salt = &data[5..21];
    let nonce = XNonce::from_slice(&data[21..45]);
    let key = derive_key(passphrase, salt)?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    cipher
        .decrypt(nonce, &data[45..])
        .map_err(|_| OpsError::Crypto("decryption failed (wrong key or corrupted backup)".into()))
}

pub fn gzip(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::write::GzEncoder;
    use std::io::Write;
    let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).map_err(|e| OpsError::Crypto(e.to_string()))?;
    encoder.finish().map_err(|e| OpsError::Crypto(e.to_string()))
}

pub fn gunzip(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    let mut out = Vec::new();
    GzDecoder::new(data)
        .read_to_end(&mut out)
        .map_err(|e| OpsError::Crypto(e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let sealed = encrypt("passphrase-1", b"hello backup").unwrap();
        assert_eq!(decrypt("passphrase-1", &sealed).unwrap(), b"hello backup");
        assert!(decrypt("wrong", &sealed).is_err());
        assert!(decrypt("passphrase-1", b"garbage").is_err());
    }

    #[test]
    fn gzip_roundtrip() {
        let data = b"compress me".repeat(100);
        assert_eq!(gunzip(&gzip(&data).unwrap()).unwrap(), data);
    }
}
