//! 密钥库占位符注入：deploy / 同步路径

use agenthub_core::secrets::inject_secrets_into_env;
use agenthub_core::store::Store;
use std::collections::HashMap;
use tempfile::tempdir;

#[test]
fn inject_replaces_placeholder_from_vault() {
    let mut env = serde_json::Map::new();
    env.insert(
        "API_KEY".into(),
        serde_json::Value::String("__AGENTHUB_SECRET__:API_KEY".into()),
    );
    env.insert("MODEL".into(), serde_json::Value::String("gpt".into()));
    let mut secrets = HashMap::new();
    secrets.insert("API_KEY".into(), "sk-live-1234".into());
    let out = inject_secrets_into_env(&env, &secrets);
    assert_eq!(out["API_KEY"].as_str().unwrap(), "sk-live-1234");
    assert_eq!(out["MODEL"].as_str().unwrap(), "gpt");
}

#[test]
fn inject_keeps_placeholder_when_missing() {
    let mut env = serde_json::Map::new();
    env.insert(
        "API_KEY".into(),
        serde_json::Value::String("__AGENTHUB_SECRET__:API_KEY".into()),
    );
    let secrets = HashMap::new();
    let out = inject_secrets_into_env(&env, &secrets);
    assert_eq!(
        out["API_KEY"].as_str().unwrap(),
        "__AGENTHUB_SECRET__:API_KEY"
    );
}

#[test]
fn vault_roundtrip_feeds_inject() {
    let dir = tempdir().unwrap();
    let store = Store::open(&dir.path().join("t.db")).unwrap();
    store.save_secret("GITHUB_TOKEN", "ghp_example").unwrap();
    let secrets = store.get_all_secrets().unwrap();
    let mut env = serde_json::Map::new();
    env.insert(
        "GITHUB_TOKEN".into(),
        serde_json::Value::String("__AGENTHUB_SECRET__:GITHUB_TOKEN".into()),
    );
    let out = inject_secrets_into_env(&env, &secrets);
    assert_eq!(out["GITHUB_TOKEN"].as_str().unwrap(), "ghp_example");
}
