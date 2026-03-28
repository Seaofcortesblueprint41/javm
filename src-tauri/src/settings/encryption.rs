use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

use super::AppSettings;

// 简单的XOR加密密钥
const ENCRYPTION_KEY: &[u8] = b"javm_secure_key_2024";

// 简单的XOR加密/解密
fn xor_cipher(data: &[u8], key: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(i, &byte)| byte ^ key[i % key.len()])
        .collect()
}

/// 加密API Key
pub fn encrypt_api_key(api_key: &str) -> String {
    let encrypted = xor_cipher(api_key.as_bytes(), ENCRYPTION_KEY);
    BASE64.encode(encrypted)
}

/// 解密API Key
pub fn decrypt_api_key(encrypted: &str) -> Result<String, String> {
    let decoded = BASE64.decode(encrypted).map_err(|e| e.to_string())?;
    let decrypted = xor_cipher(&decoded, ENCRYPTION_KEY);
    String::from_utf8(decrypted).map_err(|e| e.to_string())
}

/// 加密设置中的所有API Key
pub(super) fn encrypt_settings(settings: &mut AppSettings) {
    for provider in &mut settings.ai.providers {
        if !provider.api_key.is_empty() && !provider.api_key.starts_with("enc:") {
            provider.api_key = format!("enc:{}", encrypt_api_key(&provider.api_key));
        }
    }
}

/// 解密设置中的所有API Key
pub(super) fn decrypt_settings(settings: &mut AppSettings) {
    for provider in &mut settings.ai.providers {
        if let Some(encrypted) = provider.api_key.strip_prefix("enc:") {
            if let Ok(decrypted) = decrypt_api_key(encrypted) {
                provider.api_key = decrypted;
            }
        }
    }
}
