use agenthub_core::secrets::{
    inject_secrets_into_env, is_sensitive_env_key, mask_secret, sanitize_mcp_env_for_sync,
    SecretBackup,
};
use agenthub_core::store::Store;
use std::collections::HashMap;
use tempfile::tempdir;

#[test]
fn save_get_roundtrip() {
    let dir = tempdir().unwrap();
    let store = Store::open(&dir.path().join("t.db")).unwrap();
    store.save_secret("OPENAI_API_KEY", "sk-abcdef123456").unwrap();
    let got = store.get_secret("OPENAI_API_KEY").unwrap().unwrap();
    assert_eq!(got, "sk-abcdef123456");
}

#[test]
fn list_returns_masked_not_plaintext() {
    let dir = tempdir().unwrap();
    let store = Store::open(&dir.path().join("t.db")).unwrap();
    store.save_secret("OPENAI_API_KEY", "sk-abcdef123456").unwrap();
    let list = store.list_secrets().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "OPENAI_API_KEY");
    assert!(list[0].masked.starts_with("sk-a"));
    assert!(list[0].masked.ends_with("3456"));
    assert!(!list[0].masked.contains("def123"));
}

#[test]
fn update_overwrites_value() {
    let dir = tempdir().unwrap();
    let store = Store::open(&dir.path().join("t.db")).unwrap();
    store.save_secret("K", "old-value-01").unwrap();
    store.save_secret("K", "new-value-02").unwrap();
    assert_eq!(store.get_secret("K").unwrap().unwrap(), "new-value-02");
}

#[test]
fn delete_secret() {
    let dir = tempdir().unwrap();
    let store = Store::open(&dir.path().join("t.db")).unwrap();
    store.save_secret("K", "v-12345678").unwrap();
    store.delete_secret("K").unwrap();
    assert!(store.get_secret("K").unwrap().is_none());
}

#[test]
fn export_import_plaintext() {
    let dir = tempdir().unwrap();
    let store = Store::open(&dir.path().join("t.db")).unwrap();
    store.save_secret("A", "value-aaaa-1111").unwrap();
    store.save_secret("B", "value-bbbb-2222").unwrap();
    let exported = store.export_secrets().unwrap();
    assert_eq!(exported.len(), 2);
    assert!(exported.iter().any(|s| s.name == "A" && s.value == "value-aaaa-1111"));

    let dir2 = tempdir().unwrap();
    let store2 = Store::open(&dir2.path().join("t2.db")).unwrap();
    let n = store2.import_secrets(&exported).unwrap();
    assert_eq!(n, 2);
    assert_eq!(store2.get_secret("A").unwrap().unwrap(), "value-aaaa-1111");
    assert_eq!(store2.get_secret("B").unwrap().unwrap(), "value-bbbb-2222");
}

#[test]
fn sensitive_key_detection() {
    assert!(is_sensitive_env_key("OPENAI_API_KEY"));
    assert!(is_sensitive_env_key("github_token"));
    assert!(!is_sensitive_env_key("MODEL"));
}

#[test]
fn sync_masks_and_placeholder() {
    let mut env = serde_json::Map::new();
    env.insert("API_KEY".into(), serde_json::Value::String("sk-real".into()));
    env.insert("MODEL".into(), serde_json::Value::String("gpt-4".into()));
    let mut secrets = HashMap::new();
    secrets.insert("API_KEY".into(), "sk-real".into());
    let (sanitized, report) = sanitize_mcp_env_for_sync(&env, &secrets);
    assert_eq!(report.masked_fields, vec!["API_KEY"]);
    assert_eq!(
        sanitized.get("API_KEY").unwrap().as_str().unwrap(),
        "__AGENTHUB_SECRET__:API_KEY"
    );
    assert_eq!(sanitized.get("MODEL").unwrap().as_str().unwrap(), "gpt-4");

    let injected = inject_secrets_into_env(&sanitized, &secrets);
    assert_eq!(
        injected.get("API_KEY").unwrap().as_str().unwrap(),
        "sk-real"
    );
}

#[test]
fn mask_secret_unicode_safe() {
    let m = mask_secret("TOKEN", "密码短值");
    assert!(m.contains("***"));
    let m2 = mask_secret("TOKEN", "sk-1234567890abcd");
    assert!(m2.contains("..."));
}

// 确保 SecretBackup 序列化形态稳定（前端 TS 对应）
#[test]
fn secret_backup_serde() {
    let b = SecretBackup {
        name: "K".into(),
        value: "v".into(),
    };
    let j = serde_json::to_string(&b).unwrap();
    assert!(j.contains("\"name\""));
    assert!(j.contains("\"value\""));
}
