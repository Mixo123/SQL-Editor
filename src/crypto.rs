//! Encryption and secure persistence of database credentials and settings.
//! Fully compatible with Python sql_editor crypto storage format.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

static KEY_CACHE: OnceLock<Vec<u8>> = OnceLock::new();

pub fn app_dir() -> PathBuf {
    let home = directories::UserDirs::new()
        .map(|ud| ud.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".sql_editor")
}

pub fn conn_file() -> PathBuf {
    app_dir().join("connections.json")
}

pub fn settings_file() -> PathBuf {
    app_dir().join("settings.json")
}

pub fn key_file() -> PathBuf {
    app_dir().join(".key")
}

pub fn ensure_app_dir() {
    let dir = app_dir();
    let _ = fs::create_dir_all(&dir);
}

pub fn load_key() -> Vec<u8> {
    ensure_app_dir();
    let path = key_file();
    if path.exists() {
        if let Ok(data) = fs::read(&path) {
            if !data.is_empty() {
                return data;
            }
        }
    }

    let mut key = vec![0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    let _ = fs::write(&path, &key);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }

    key
}

pub fn get_key() -> &'static [u8] {
    KEY_CACHE.get_or_init(load_key).as_slice()
}

pub fn xor_bytes(data: &[u8], key: &[u8]) -> Vec<u8> {
    if key.is_empty() {
        return data.to_vec();
    }
    data.iter()
        .enumerate()
        .map(|(i, &b)| b ^ key[i % key.len()])
        .collect()
}

pub fn encrypt_str(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let key = get_key();
    let raw = xor_bytes(s.as_bytes(), key);
    format!("enc:{}", BASE64.encode(raw))
}

pub fn decrypt_str(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    if !s.starts_with("enc:") {
        return s.to_string();
    }
    let b64_part = &s[4..];
    if let Ok(raw) = BASE64.decode(b64_part) {
        let key = get_key();
        let decrypted = xor_bytes(&raw, key);
        if let Ok(val) = String::from_utf8(decrypted) {
            return val;
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_roundtrip() {
        let secret = "super_secret_password_123!@#";
        let encrypted = encrypt_str(secret);
        assert!(encrypted.starts_with("enc:"));
        let decrypted = decrypt_str(&encrypted);
        assert_eq!(decrypted, secret);
    }
}

