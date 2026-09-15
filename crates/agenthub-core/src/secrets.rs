//! 密钥管理：加密存储 MCP env 中的敏感值（API Key / Token / Secret）。
//!
//! 策略（v0.2）：
//! - **Windows 主路径：DPAPI**（`CryptProtectData`，绑定当前用户，系统级保护）
//! - 非 Windows / 测试回退：随机主密钥文件 + AES-256-GCM（不再用主机名推导）
//! - 存储带版本前缀：`v2:` = DPAPI；`v1:` = 旧机器哈希 AES（可自动迁移）
//! - 同步时自动识别敏感 env 字段，替换为占位符；可本地注入真实值

use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
#[cfg(not(windows))]
use rand::Rng;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(not(windows))]
use std::path::PathBuf;

use crate::error::{CoreError, Result};
use crate::store::Store;

/// 敏感 env 字段名的匹配模式（不区分大小写）
const SENSITIVE_PATTERNS: &[&str] = &[
    "token", "key", "secret", "password", "api_key", "apikey",
    "api_secret", "access_key", "private_key", "auth_token",
    "bearer", "credential", "passphrase",
];

const ENC_PREFIX_DPAPI: &str = "v2:";
const ENC_PREFIX_LEGACY: &str = "v1:";

/// 列表展示用：不含密文
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretEntry {
    pub id: i64,
    pub name: String,
    /// 脱敏展示，如 `sk-1...cdef`
    pub masked: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 导出/导入备份条目（明文，由用户自行保管）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretBackup {
    pub name: String,
    pub value: String,
}

/// 同步时的密钥掩码报告
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSecretReport {
    pub masked_fields: Vec<String>,
    pub placeholder_count: usize,
}

/* ---------- 加密原语 ---------- */

#[cfg(windows)]
mod sys_protect {
    use super::{CoreError, Result};
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    pub fn protect(plaintext: &[u8]) -> Result<String> {
        unsafe {
            let in_blob = CRYPT_INTEGER_BLOB {
                cbData: plaintext.len() as u32,
                pbData: plaintext.as_ptr() as *mut u8,
            };
            let mut out_blob = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };
            let ok = CryptProtectData(
                &in_blob,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob,
            );
            if ok == 0 {
                return Err(CoreError::Other(format!(
                    "DPAPI 加密失败: {}",
                    std::io::Error::last_os_error()
                )));
            }
            let data =
                std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
            LocalFree(out_blob.pbData as _);
            Ok(B64.encode(&data))
        }
    }

    pub fn unprotect(ciphertext_b64: &str) -> Result<Vec<u8>> {
        let data = B64.decode(ciphertext_b64).map_err(|e| {
            CoreError::Other(format!("DPAPI 密文解码失败: {e}"))
        })?;
        unsafe {
            let in_blob = CRYPT_INTEGER_BLOB {
                cbData: data.len() as u32,
                pbData: data.as_ptr() as *mut u8,
            };
            let mut out_blob = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };
            let ok = CryptUnprotectData(
                &in_blob,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob,
            );
            if ok == 0 {
                return Err(CoreError::Other(format!(
                    "DPAPI 解密失败: {}",
                    std::io::Error::last_os_error()
                )));
            }
            let plain =
                std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
            LocalFree(out_blob.pbData as _);
            Ok(plain)
        }
    }
}

/// 非 Windows / 测试回退：随机主密钥落盘（32 字节），AES-256-GCM。
/// 不再用主机名/用户名推导——熵不足且可预测。
#[cfg(not(windows))]
fn fallback_key_file() -> PathBuf {
    crate::util::app_data_dir().join("secret.key")
}

#[cfg(not(windows))]
fn fallback_master_key() -> Result<[u8; 32]> {
    let path = fallback_key_file();
    if path.exists() {
        let bytes = std::fs::read(&path)?;
        if bytes.len() == 32 {
            let mut k = [0u8; 32];
            k.copy_from_slice(&bytes);
            return Ok(k);
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut key = [0u8; 32];
    rand::thread_rng().fill(&mut key);
    std::fs::write(&path, key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(key)
}

#[cfg(not(windows))]
fn fallback_encrypt(plaintext: &str) -> Result<String> {
    let key = fallback_master_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| CoreError::Other(format!("加密初始化失败: {e}")))?;
    let mut rng = rand::thread_rng();
    let nonce_bytes: [u8; 12] = rng.gen();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| CoreError::Other(format!("加密失败: {e}")))?;
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(format!("{ENC_PREFIX_DPAPI}{}", B64.encode(&combined)))
}

#[cfg(not(windows))]
fn fallback_decrypt(stored: &str) -> Result<String> {
    let body = stored.strip_prefix(ENC_PREFIX_DPAPI).unwrap_or(stored);
    let combined = B64
        .decode(body)
        .map_err(|e| CoreError::Other(format!("解码失败: {e}")))?;
    if combined.len() < 12 {
        return Err(CoreError::Other("加密数据格式错误".into()));
    }
    let key = fallback_master_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| CoreError::Other(format!("解密初始化失败: {e}")))?;
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| CoreError::Other(format!("解密失败: {e}")))?;
    String::from_utf8(plaintext).map_err(|e| CoreError::Other(format!("UTF-8 解码失败: {e}")))
}

/// v1 遗留：主机名+用户名推导（仅用于迁移读取，不再写入）
#[allow(dead_code)]
fn legacy_machine_key() -> [u8; 32] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    if let Ok(name) = hostname::get() {
        name.to_string_lossy().hash(&mut hasher);
    }
    if let Some(user) = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .ok()
    {
        user.hash(&mut hasher);
    }
    "AgentHub-SecretSalt-v1".hash(&mut hasher);
    let hash = hasher.finish().to_le_bytes();
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&hash);
    for i in 0..32 {
        key[i] = key[i % 8].wrapping_add(i as u8);
    }
    key
}

fn legacy_decrypt(stored: &str) -> Result<String> {
    let body = stored.strip_prefix(ENC_PREFIX_LEGACY).unwrap_or(stored);
    let combined = B64
        .decode(body)
        .map_err(|e| CoreError::Other(format!("解码失败: {e}")))?;
    if combined.len() < 12 {
        return Err(CoreError::Other("加密数据格式错误".into()));
    }
    let key = legacy_machine_key();
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| CoreError::Other(format!("解密初始化失败: {e}")))?;
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| CoreError::Other(format!("解密失败: {e}")))?;
    String::from_utf8(plaintext).map_err(|e| CoreError::Other(format!("UTF-8 解码失败: {e}")))
}

fn encrypt_value(plaintext: &str) -> Result<String> {
    #[cfg(windows)]
    {
        let blob = sys_protect::protect(plaintext.as_bytes())?;
        return Ok(format!("{ENC_PREFIX_DPAPI}{blob}"));
    }
    #[cfg(not(windows))]
    {
        fallback_encrypt(plaintext)
    }
}

fn decrypt_value(stored: &str) -> Result<String> {
    // v2: Windows DPAPI 或 非 Windows 随机密钥 AES
    if let Some(rest) = stored.strip_prefix(ENC_PREFIX_DPAPI) {
        #[cfg(windows)]
        {
            let bytes = sys_protect::unprotect(rest)?;
            return String::from_utf8(bytes)
                .map_err(|e| CoreError::Other(format!("UTF-8 解码失败: {e}")));
        }
        #[cfg(not(windows))]
        {
            let _ = rest;
            return fallback_decrypt(stored);
        }
    }
    // v1: 遗留机器哈希 AES —— 只读迁移
    if stored.starts_with(ENC_PREFIX_LEGACY) {
        return legacy_decrypt(stored);
    }
    // 无前缀：按遗留格式尝试（历史数据）
    legacy_decrypt(stored)
}

fn mask_for_display(plaintext: &str) -> String {
    mask_secret("VALUE", plaintext)
}

/* ---------- 业务辅助 ---------- */

pub fn is_sensitive_env_key(key: &str) -> bool {
    let lower = key.to_lowercase();
    SENSITIVE_PATTERNS.iter().any(|pattern| lower.contains(pattern))
}

pub fn mask_secret(name: &str, value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 8 {
        let head: String = name.chars().take(4).collect();
        format!("***{head}***")
    } else {
        let start: String = chars.iter().take(4).collect();
        let end: String = chars.iter().skip(chars.len() - 4).collect();
        format!("{start}...{end}")
    }
}

/// 同步 MCP env 时自动掩码敏感字段
pub fn sanitize_mcp_env_for_sync(
    env: &serde_json::Map<String, serde_json::Value>,
    secrets: &HashMap<String, String>,
) -> (serde_json::Map<String, serde_json::Value>, SyncSecretReport) {
    let mut sanitized = serde_json::Map::new();
    let mut masked_fields = Vec::new();
    let mut placeholder_count = 0;

    for (key, value) in env {
        if is_sensitive_env_key(key) {
            if secrets.contains_key(key) {
                sanitized.insert(
                    key.clone(),
                    serde_json::Value::String(format!("__AGENTHUB_SECRET__:{key}")),
                );
                masked_fields.push(key.clone());
                placeholder_count += 1;
            } else if let Some(v) = value.as_str() {
                sanitized.insert(key.clone(), value.clone());
                if v.starts_with("__") {
                    placeholder_count += 1;
                }
            } else {
                sanitized.insert(key.clone(), value.clone());
            }
        } else {
            sanitized.insert(key.clone(), value.clone());
        }
    }

    (
        sanitized,
        SyncSecretReport {
            masked_fields,
            placeholder_count,
        },
    )
}

/// 在写入 Agent 配置前注入真实密钥值
pub fn inject_secrets_into_env(
    env: &serde_json::Map<String, serde_json::Value>,
    secrets: &HashMap<String, String>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut result = serde_json::Map::new();

    for (key, value) in env {
        if let Some(v) = value.as_str() {
            if let Some(real_key) = v.strip_prefix("__AGENTHUB_SECRET__:") {
                if let Some(real_value) = secrets.get(real_key) {
                    result.insert(key.clone(), serde_json::Value::String(real_value.clone()));
                } else {
                    result.insert(key.clone(), value.clone());
                }
            } else {
                result.insert(key.clone(), value.clone());
            }
        } else {
            result.insert(key.clone(), value.clone());
        }
    }

    result
}

/* ---------- SQLite 持久化 ---------- */

impl Store {
    pub fn init_secrets_table(&self) -> Result<()> {
        self.connection().execute_batch(
            "CREATE TABLE IF NOT EXISTS secrets (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL UNIQUE,
                encrypted_value TEXT NOT NULL,
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL
            );"
        )?;
        Ok(())
    }

    pub fn save_secret(&self, name: &str, plaintext: &str) -> Result<i64> {
        self.init_secrets_table()?;
        let encrypted = encrypt_value(plaintext)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        self.connection().execute(
            "INSERT INTO secrets(name, encrypted_value, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?3)
             ON CONFLICT(name) DO UPDATE SET encrypted_value = excluded.encrypted_value, updated_at = excluded.updated_at",
            rusqlite::params![name, encrypted, now],
        )?;

        Ok(self.connection().last_insert_rowid())
    }

    pub fn get_secret(&self, name: &str) -> Result<Option<String>> {
        self.init_secrets_table()?;
        let result = self
            .connection()
            .query_row(
                "SELECT encrypted_value FROM secrets WHERE name = ?1",
                [name],
                |r| r.get::<_, String>(0),
            )
            .optional()?;

        match result {
            Some(encrypted) => {
                let plain = decrypt_value(&encrypted)?;
                // 读到遗留/非当前前缀密文时静默升级
                if !encrypted.starts_with(ENC_PREFIX_DPAPI) {
                    let upgraded = encrypt_value(&plain)?;
                    let _ = self.connection().execute(
                        "UPDATE secrets SET encrypted_value = ?1, updated_at = ?2 WHERE name = ?3",
                        rusqlite::params![
                            upgraded,
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_millis() as i64)
                                .unwrap_or(0),
                            name
                        ],
                    );
                }
                Ok(Some(plain))
            }
            None => Ok(None),
        }
    }

    /// 列出密钥（仅脱敏，绝不回传密文）
    pub fn list_secrets(&self) -> Result<Vec<SecretEntry>> {
        self.init_secrets_table()?;
        let mut stmt = self.connection().prepare(
            "SELECT id, name, encrypted_value, created_at, updated_at FROM secrets ORDER BY name",
        )?;

        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (id, name, encrypted, created_at, updated_at) = row?;
            let masked = decrypt_value(&encrypted)
                .map(|p| mask_for_display(&p))
                .unwrap_or_else(|_| "（无法解密）".into());
            out.push(SecretEntry {
                id,
                name,
                masked,
                created_at,
                updated_at,
            });
        }
        Ok(out)
    }

    pub fn delete_secret(&self, name: &str) -> Result<()> {
        self.init_secrets_table()?;
        self.connection()
            .execute("DELETE FROM secrets WHERE name = ?1", [name])?;
        Ok(())
    }

    pub fn get_all_secrets(&self) -> Result<HashMap<String, String>> {
        self.init_secrets_table()?;
        let mut stmt = self
            .connection()
            .prepare("SELECT name, encrypted_value FROM secrets")?;
        let rows = stmt.query_map([], |r| {
            let name: String = r.get(0)?;
            let encrypted: String = r.get(1)?;
            Ok((name, encrypted))
        })?;

        let mut secrets = HashMap::new();
        for row in rows {
            let (name, encrypted) = row?;
            if let Ok(value) = decrypt_value(&encrypted) {
                secrets.insert(name, value);
            }
        }
        Ok(secrets)
    }

    /// 导出为明文备份（用户自行保管；跨机器可用）
    pub fn export_secrets(&self) -> Result<Vec<SecretBackup>> {
        self.init_secrets_table()?;
        let all = self.get_all_secrets()?;
        let mut out: Vec<SecretBackup> = all
            .into_iter()
            .map(|(name, value)| SecretBackup { name, value })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// 从明文备份导入（按当前平台策略加密）
    pub fn import_secrets(&self, entries: &[SecretBackup]) -> Result<usize> {
        self.init_secrets_table()?;
        let mut count = 0;
        for entry in entries {
            self.save_secret(&entry.name, &entry.value)?;
            count += 1;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = "sk-test-1234567890abcdef";
        let encrypted = encrypt_value(plaintext).unwrap();
        assert!(encrypted.starts_with(ENC_PREFIX_DPAPI));
        let decrypted = decrypt_value(&encrypted).unwrap();
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_is_sensitive_env_key() {
        assert!(is_sensitive_env_key("OPENAI_API_KEY"));
        assert!(is_sensitive_env_key("AUTH_TOKEN"));
        assert!(is_sensitive_env_key("SECRET_KEY"));
        assert!(is_sensitive_env_key("PASSWORD"));
        assert!(!is_sensitive_env_key("MODEL_NAME"));
        assert!(!is_sensitive_env_key("PORT"));
    }

    #[test]
    fn test_mask_secret() {
        let masked = mask_secret("API_KEY", "sk-1234567890abcdef");
        assert!(masked.starts_with("sk-1"));
        assert!(masked.ends_with("cdef"));
        // 不泄露中间段
        assert!(!masked.contains("4567890"));
    }

    #[test]
    fn test_mask_short_value_no_panic() {
        let _ = mask_secret("X", "短");
        let _ = mask_secret("OPENAI", "abcdef");
    }
}
