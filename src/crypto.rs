//! Encryption of the VTTL passwords that the server keeps for a session, and the
//! hashing of session tokens.
//!
//! The key comes from `SECRET_KEY` (base64, 32 bytes). Without it, the server makes
//! a key file next to the database on the first start. Losing the key only logs
//! everybody out: a session that cannot be decrypted is treated as missing.

use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use sha2::{Digest, Sha256};
use std::path::Path;

pub struct Crypto {
    cipher: ChaCha20Poly1305,
}

impl Crypto {
    pub fn load(env_key: Option<String>, key_file: &Path) -> Result<Self> {
        let key = match env_key.filter(|k| !k.is_empty()) {
            Some(k) => STANDARD.decode(k.trim()).context("SECRET_KEY is not valid base64")?,
            None if key_file.exists() => STANDARD.decode(std::fs::read_to_string(key_file)?.trim())?,
            None => {
                let k: [u8; 32] = rand::random();
                write_private(key_file, &STANDARD.encode(k))?;
                tracing::info!("created a new secret key in {}", key_file.display());
                k.to_vec()
            }
        };
        if key.len() != 32 {
            bail!("the secret key must be 32 bytes");
        }
        Ok(Self { cipher: ChaCha20Poly1305::new(Key::from_slice(&key)) })
    }

    pub fn encrypt(&self, plain: &str) -> String {
        let nonce: [u8; 12] = rand::random();
        let mut out = nonce.to_vec();
        out.extend(self.cipher.encrypt(Nonce::from_slice(&nonce), plain.as_bytes()).expect("encrypt"));
        STANDARD.encode(out)
    }

    pub fn decrypt(&self, sealed: &str) -> Option<String> {
        let raw = STANDARD.decode(sealed).ok()?;
        if raw.len() < 12 {
            return None;
        }
        let (nonce, data) = raw.split_at(12);
        let plain = self.cipher.decrypt(Nonce::from_slice(nonce), data).ok()?;
        String::from_utf8(plain).ok()
    }
}

/// A new random session token (sent to the browser) and its hash (kept in the database).
pub fn new_token() -> (String, String) {
    let t: [u8; 32] = rand::random();
    let token = URL_SAFE_NO_PAD.encode(t);
    let hash = token_hash(&token);
    (token, hash)
}

pub fn token_hash(token: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
}

fn write_private(path: &Path, content: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?;
        f.write_all(content.as_bytes())?;
    }
    #[cfg(not(unix))]
    std::fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("vl-crypto-{}", rand::random::<u32>()));
        let c = Crypto::load(None, &dir.join("key")).unwrap();
        let s = c.encrypt("geheim & <xml>");
        assert_ne!(s, c.encrypt("geheim & <xml>"), "nonce must differ");
        assert_eq!(c.decrypt(&s).as_deref(), Some("geheim & <xml>"));
        // Same key file gives the same key.
        let c2 = Crypto::load(None, &dir.join("key")).unwrap();
        assert_eq!(c2.decrypt(&s).as_deref(), Some("geheim & <xml>"));
        assert!(c.decrypt("bm9wZQ==").is_none());
        std::fs::remove_dir_all(dir).ok();
    }
}
