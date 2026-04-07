use codex_switch_lib::codex_accounts::auth_file::{
    parse_codex_account_from_auth_json, write_live_codex_auth_json,
};
use codex_switch_lib::codex_accounts::{
    codex_accounts_store_path, CodexAccountRecord, CodexAccountStore, CodexDeleteAccountError,
    CodexQuotaSnapshot, StoredCodexAccounts,
};
use codex_switch_lib::codex_accounts::watcher::{
    poll_codex_auth_change, AuthFileDebouncer, sync_live_auth_once,
};

#[path = "support.rs"]
mod support;

use base64::Engine;
use serde_json::json;
use support::{ensure_test_home, reset_test_fs, test_mutex};
use tempfile::tempdir;
use std::time::{Duration, Instant};

fn jwt(payload: serde_json::Value) -> String {
    let header =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_string(&payload).expect("serialize payload"));
    format!("{header}.{payload}.signature")
}

fn sample_live_auth(email: &str, account_id: &str) -> serde_json::Value {
    json!({
        "OPENAI_API_KEY": null,
        "auth_mode": "chatgpt",
        "last_refresh": "2026-04-06T10:00:00Z",
        "tokens": {
            "access_token": "header.payload.signature",
            "refresh_token": "refresh-token",
            "id_token": "header.payload.signature",
            "account_id": account_id
        },
        "profile": {
            "email": email
        },
        "plan_type": "plus",
        "display_name": "Alex",
        "avatar_seed": "seed",
        "unknown": {
            "preserve_me": true
        }
    })
}

fn sample_live_auth_with_jwt_claims() -> serde_json::Value {
    json!({
        "OPENAI_API_KEY": null,
        "auth_mode": "chatgpt",
        "last_refresh": "2026-04-06T10:00:00Z",
        "tokens": {
            "access_token": jwt(json!({
                "account_id": "7a569f43-4a34-45c4-8190-9580bdf63fab",
                "email": "earnzh@gmail.com",
                "plan": "plus"
            })),
            "refresh_token": "refresh-token",
            "id_token": jwt(json!({
                "email": "earnzh@gmail.com",
                "plan_type": "plus",
                "account_id": "7a569f43-4a34-45c4-8190-9580bdf63fab"
            }))
        },
        "unknown": {
            "preserve_me": true
        }
    })
}

fn sample_live_auth_with_nested_chatgpt_account_claim() -> serde_json::Value {
    json!({
        "access_token": jwt(json!({
            "sub": "google-oauth2|105324430781663303950",
            "email": "earnzh@gmail.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "7a569f43-4a34-45c4-8190-9580bdf63fab",
                "chatgpt_plan_type": "plus"
            }
        })),
        "id_token": jwt(json!({
            "sub": "google-oauth2|105324430781663303950",
            "email": "earnzh@gmail.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "7a569f43-4a34-45c4-8190-9580bdf63fab",
                "chatgpt_plan_type": "plus"
            }
        })),
        "refresh_token": "refresh-token"
    })
}

fn sample_top_level_oauth_token_response() -> serde_json::Value {
    json!({
        "access_token": jwt(json!({
            "sub": "acct-top-level",
            "account_id": "acct-top-level",
            "email": "top-level@example.com",
            "plan_type": "plus",
            "name": "Top Level User"
        })),
        "refresh_token": "refresh-token",
        "id_token": jwt(json!({
            "sub": "acct-top-level",
            "email": "top-level@example.com",
            "plan_type": "plus",
            "name": "Top Level User"
        })),
        "token_type": "Bearer",
        "expires_in": 3600
    })
}

fn make_record(account_id: &str, is_active: bool) -> CodexAccountRecord {
    CodexAccountRecord {
        id: format!("record-{account_id}"),
        email: Some(format!("{account_id}@example.com")),
        account_id: Some(account_id.to_string()),
        plan_type: Some("plus".to_string()),
        display_name: Some(format!("Display {account_id}")),
        avatar_seed: format!("seed-{account_id}"),
        added_at: 1000,
        last_used_at: Some(2000),
        is_active,
        auth_json: json!({
            "tokens": { "account_id": account_id },
            "unknown": { "keep": true }
        }),
        quota: Some(CodexQuotaSnapshot {
            five_hour_percent: Some(12.5),
            five_hour_reset_at: Some(123),
            week_percent: Some(45.0),
            week_reset_at: Some(456),
            refreshed_at: 789,
            last_error: None,
        }),
        metadata: Default::default(),
    }
}

fn sample_quota_snapshot(five_hour_percent: f64, week_percent: f64) -> CodexQuotaSnapshot {
    CodexQuotaSnapshot {
        five_hour_percent: Some(five_hour_percent),
        five_hour_reset_at: Some(123),
        week_percent: Some(week_percent),
        week_reset_at: Some(456),
        refreshed_at: 789,
        last_error: None,
    }
}

#[test]
fn codex_accounts_store_path_uses_app_config_dir() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    assert_eq!(
        codex_accounts_store_path(),
        home.join(".codex-switch").join("codex-accounts.json")
    );
}

#[test]
fn codex_accounts_store_bootstrap_imports_live_auth_as_active_account() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let live_auth_path = home.join(".codex").join("auth.json");
    std::fs::create_dir_all(live_auth_path.parent().expect("auth parent"))
        .expect("create codex dir");
    std::fs::write(
        &live_auth_path,
        serde_json::to_string_pretty(&sample_live_auth("active@example.com", "acct-live"))
            .expect("serialize auth"),
    )
    .expect("write live auth");

    let mut store = CodexAccountStore::new(home.join(".codex-switch").join("codex-accounts.json"));
    let imported = store
        .bootstrap_from_live_auth(&live_auth_path)
        .expect("bootstrap from live auth");

    assert_eq!(imported.email.as_deref(), Some("active@example.com"));
    assert_eq!(imported.account_id.as_deref(), Some("acct-live"));
    assert_eq!(imported.plan_type.as_deref(), Some("plus"));
    assert_eq!(imported.display_name.as_deref(), Some("Alex"));
    assert!(imported.is_active);
    assert_eq!(imported.auth_json["unknown"]["preserve_me"], json!(true));

    let accounts = store.list_accounts().expect("list accounts");
    assert_eq!(accounts.len(), 1);
    assert!(accounts[0].is_active);
}

#[test]
fn codex_accounts_store_bootstrap_preserves_existing_quota_for_active_account() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let live_auth_path = home.join(".codex").join("auth.json");
    std::fs::create_dir_all(live_auth_path.parent().expect("auth parent"))
        .expect("create codex dir");
    std::fs::write(
        &live_auth_path,
        serde_json::to_string_pretty(&sample_live_auth("active@example.com", "acct-live"))
            .expect("serialize auth"),
    )
    .expect("write live auth");

    let store_path = home.join(".codex-switch").join("codex-accounts.json");
    let mut store = CodexAccountStore::new(store_path.clone());
    let imported = store
        .bootstrap_from_live_auth(&live_auth_path)
        .expect("bootstrap from live auth");

    let mut stored = store.load().expect("load store");
    let active = stored
        .accounts
        .iter_mut()
        .find(|account| account.account_id.as_deref() == Some("acct-live"))
        .expect("active account exists");
    active.quota = Some(sample_quota_snapshot(21.0, 63.0));
    store.save(&stored).expect("save quota snapshot");

    let reimported = store
        .bootstrap_from_live_auth(&live_auth_path)
        .expect("re-bootstrap from live auth");

    assert_eq!(imported.account_id.as_deref(), Some("acct-live"));
    assert_eq!(reimported.account_id.as_deref(), Some("acct-live"));
    assert_eq!(
        reimported.quota.as_ref().and_then(|quota| quota.five_hour_percent),
        Some(21.0)
    );
    assert_eq!(
        reimported.quota.as_ref().and_then(|quota| quota.week_percent),
        Some(63.0)
    );
}

#[test]
fn parse_live_auth_extracts_email_plan_and_account_id() {
    let parsed = parse_codex_account_from_auth_json(sample_live_auth_with_jwt_claims())
        .expect("parse live auth");

    assert_eq!(parsed.email.as_deref(), Some("earnzh@gmail.com"));
    assert_eq!(parsed.plan_type.as_deref(), Some("plus"));
    assert_eq!(
        parsed.account_id.as_deref(),
        Some("7a569f43-4a34-45c4-8190-9580bdf63fab")
    );
    assert_eq!(parsed.auth_json["unknown"]["preserve_me"], json!(true));
}

#[test]
fn parse_live_auth_prefers_nested_chatgpt_account_id_over_sub_claim() {
    let parsed = parse_codex_account_from_auth_json(
        sample_live_auth_with_nested_chatgpt_account_claim(),
    )
    .expect("parse live auth");

    assert_eq!(parsed.email.as_deref(), Some("earnzh@gmail.com"));
    assert_eq!(parsed.plan_type.as_deref(), Some("plus"));
    assert_eq!(
        parsed.account_id.as_deref(),
        Some("7a569f43-4a34-45c4-8190-9580bdf63fab")
    );
}

#[test]
fn parse_live_auth_prefers_direct_fields_over_jwt_claims() {
    let parsed = parse_codex_account_from_auth_json(json!({
        "OPENAI_API_KEY": null,
        "auth_mode": "chatgpt",
        "tokens": {
            "access_token": jwt(json!({
                "account_id": "acct-from-jwt",
                "email": "jwt@example.com",
                "plan": "enterprise"
            })),
            "id_token": jwt(json!({
                "account_id": "acct-from-jwt",
                "email": "jwt@example.com",
                "plan_type": "enterprise"
            })),
            "refresh_token": "refresh-token",
            "account_id": "acct-direct"
        },
        "profile": {
            "email": "direct@example.com"
        },
        "plan_type": "plus"
    }))
    .expect("parse live auth");

    assert_eq!(parsed.account_id.as_deref(), Some("acct-direct"));
    assert_eq!(parsed.email.as_deref(), Some("direct@example.com"));
    assert_eq!(parsed.plan_type.as_deref(), Some("plus"));
}

#[test]
fn parse_live_auth_preserves_unknown_fields_when_reconstructed() {
    let parsed = parse_codex_account_from_auth_json(sample_live_auth_with_jwt_claims())
        .expect("parse live auth");

    assert_eq!(parsed.auth_json["unknown"]["preserve_me"], json!(true));
    assert_eq!(
        parsed.auth_json["tokens"]["refresh_token"],
        json!("refresh-token")
    );
}

#[test]
fn parse_live_auth_writes_stored_account_back_to_live_auth_json() {
    let dir = tempdir().expect("tempdir");
    let live_auth_path = dir.path().join("auth.json");
    let account = CodexAccountRecord {
        id: "record-1".to_string(),
        email: Some("earnzh@gmail.com".to_string()),
        account_id: Some("7a569f43-4a34-45c4-8190-9580bdf63fab".to_string()),
        plan_type: Some("plus".to_string()),
        display_name: Some("Alex".to_string()),
        avatar_seed: "seed".to_string(),
        added_at: 1000,
        last_used_at: Some(2000),
        is_active: true,
        auth_json: sample_live_auth_with_jwt_claims(),
        quota: None,
        metadata: Default::default(),
    };

    write_live_codex_auth_json(&live_auth_path, &account).expect("write live auth");

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&live_auth_path).expect("read written auth"))
            .expect("parse written auth");
    assert_eq!(written["unknown"]["preserve_me"], json!(true));
    assert_eq!(
        written["tokens"]["access_token"],
        account.auth_json["tokens"]["access_token"]
    );
    assert_eq!(
        written["tokens"]["account_id"],
        json!("7a569f43-4a34-45c4-8190-9580bdf63fab")
    );
    assert_eq!(written["profile"]["email"], json!("earnzh@gmail.com"));
    assert_eq!(written["plan_type"], json!("plus"));
}

#[test]
fn parse_live_auth_write_rehydrates_null_fields_from_account_record() {
    let dir = tempdir().expect("tempdir");
    let live_auth_path = dir.path().join("auth.json");
    let account = CodexAccountRecord {
        id: "record-null".to_string(),
        email: Some("restored@example.com".to_string()),
        account_id: Some("acct-null".to_string()),
        plan_type: Some("plus".to_string()),
        display_name: Some("Restored".to_string()),
        avatar_seed: "restored-seed".to_string(),
        added_at: 1000,
        last_used_at: Some(2000),
        is_active: true,
        auth_json: json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": "access-token",
                "refresh_token": "refresh-token",
                "account_id": "acct-null"
            },
            "profile": {
                "email": null
            },
            "plan_type": null,
            "display_name": null,
            "avatar_seed": null
        }),
        quota: None,
        metadata: Default::default(),
    };

    write_live_codex_auth_json(&live_auth_path, &account).expect("write live auth");

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&live_auth_path).expect("read written auth"))
            .expect("parse written auth");
    assert_eq!(written["profile"]["email"], json!("restored@example.com"));
    assert_eq!(written["plan_type"], json!("plus"));
    assert_eq!(written["display_name"], json!("Restored"));
    assert_eq!(written["avatar_seed"], json!("restored-seed"));
}

#[test]
fn write_live_auth_normalizes_top_level_oauth_token_response_to_codex_shape() {
    let dir = tempdir().expect("tempdir");
    let live_auth_path = dir.path().join("auth.json");
    let account = parse_codex_account_from_auth_json(sample_top_level_oauth_token_response())
        .expect("parse top-level oauth token response");

    write_live_codex_auth_json(&live_auth_path, &account).expect("write normalized live auth");

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&live_auth_path).expect("read written auth"))
            .expect("parse written auth");
    assert_eq!(written["auth_mode"], json!("chatgpt"));
    assert_eq!(written["tokens"]["account_id"], json!("acct-top-level"));
    assert_eq!(written["profile"]["email"], json!("top-level@example.com"));
    assert_eq!(written["plan_type"], json!("plus"));
    assert_eq!(written["display_name"], json!("Top Level User"));
    assert!(written["last_refresh"].as_str().is_some());
    assert!(written["tokens"]["access_token"].as_str().is_some());
    assert!(written["tokens"]["refresh_token"].as_str().is_some());
    assert!(written["tokens"]["id_token"].as_str().is_some());
    assert!(
        written.get("access_token").is_none(),
        "top-level access_token should be normalized into tokens.access_token"
    );
    assert!(
        written.get("refresh_token").is_none(),
        "top-level refresh_token should be normalized into tokens.refresh_token"
    );
    assert!(
        written.get("id_token").is_none(),
        "top-level id_token should be normalized into tokens.id_token"
    );
}

#[test]
fn codex_accounts_store_preserves_corrupt_store_file_until_repaired() {
    let dir = tempdir().expect("tempdir");
    let store_path = dir.path().join("codex-accounts.json");
    std::fs::write(&store_path, "{not-valid-json").expect("write corrupt store");

    let mut store = CodexAccountStore::new(store_path.clone());
    let load_error = store.load().expect_err("corrupt store should fail to load");
    assert!(load_error.to_string().contains("unreadable"));
    let list_error = store
        .list_accounts()
        .expect_err("corrupt store should fail to list accounts");
    assert!(list_error.to_string().contains("unreadable"));

    let upsert_error = store
        .upsert_account(sample_live_auth("corrupt@example.com", "acct-corrupt"))
        .expect_err("corrupt store should block writes");
    assert!(upsert_error.to_string().contains("unreadable"));
    assert_eq!(
        std::fs::read_to_string(&store_path).expect("read corrupt store"),
        "{not-valid-json"
    );
}

#[test]
fn codex_accounts_store_upsert_replaces_matching_account_id_without_creating_duplicates() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let store_path = home.join(".codex-switch").join("codex-accounts.json");
    let mut store = CodexAccountStore::new(store_path);

    let first = store
        .upsert_account(sample_live_auth("first@example.com", "acct-1"))
        .expect("insert first");
    let second = store
        .upsert_account(sample_live_auth("second@example.com", "acct-1"))
        .expect("replace first");

    assert_eq!(first.id, second.id);
    let accounts = store.list_accounts().expect("list accounts");
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].email.as_deref(), Some("second@example.com"));
}

#[test]
fn codex_accounts_store_set_active_account_clears_previous_active_flag() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let store_path = home.join(".codex-switch").join("codex-accounts.json");
    let mut store = CodexAccountStore::new(store_path);

    let first = store
        .upsert_account(sample_live_auth("first@example.com", "acct-1"))
        .expect("insert first");
    let second = store
        .upsert_account(sample_live_auth("second@example.com", "acct-2"))
        .expect("insert second");

    store
        .set_active_account(&first.id)
        .expect("activate first account");
    store
        .set_active_account(&second.id)
        .expect("activate second account");

    let accounts = store.list_accounts().expect("list accounts");
    let first_record = accounts
        .iter()
        .find(|account| account.id == first.id)
        .unwrap();
    let second_record = accounts
        .iter()
        .find(|account| account.id == second.id)
        .unwrap();

    assert!(!first_record.is_active);
    assert!(second_record.is_active);
}

#[test]
fn codex_accounts_store_deleting_active_account_is_rejected() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let store_path = home.join(".codex-switch").join("codex-accounts.json");
    let mut store = CodexAccountStore::new(store_path);

    let account = store
        .upsert_account(sample_live_auth("active@example.com", "acct-active"))
        .expect("insert account");

    store
        .set_active_account(&account.id)
        .expect("activate account");

    let err = store
        .delete_account(&account.id)
        .expect_err("deleting active account should fail");

    assert!(matches!(err, CodexDeleteAccountError::ActiveAccount));
}

#[test]
fn codex_accounts_store_file_round_trips_saved_accounts() {
    let dir = tempdir().expect("tempdir");
    let mut store = CodexAccountStore::new(dir.path().join("codex-accounts.json"));
    let original = StoredCodexAccounts {
        accounts: vec![make_record("acct-saved", true)],
    };

    store.save(&original).expect("save accounts");
    let reloaded = store.load().expect("reload store file");

    assert_eq!(reloaded.accounts.len(), 1);
    assert_eq!(
        reloaded.accounts[0].email.as_deref(),
        Some("acct-saved@example.com")
    );
    assert_eq!(
        reloaded.accounts[0].auth_json["unknown"]["keep"],
        json!(true)
    );
}

#[test]
fn codex_accounts_store_upserting_active_record_deactivates_previous_active_account() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let store_path = home.join(".codex-switch").join("codex-accounts.json");
    let mut store = CodexAccountStore::new(store_path);
    store
        .upsert_account(make_record("acct-1", true))
        .expect("insert first active");
    store
        .upsert_account(make_record("acct-2", true))
        .expect("insert second active");

    let accounts = store.list_accounts().expect("list accounts");
    assert_eq!(
        accounts.iter().filter(|account| account.is_active).count(),
        1
    );
    assert!(
        accounts
            .iter()
            .find(|account| account.account_id.as_deref() == Some("acct-2"))
            .expect("second account")
            .is_active
    );
}

#[test]
fn codex_auth_watcher_syncs_live_auth_changes_into_store_and_emits_update_event() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let store_path = home.join(".codex-switch").join("codex-accounts.json");
    let live_auth_path = home.join(".codex").join("auth.json");
    std::fs::create_dir_all(live_auth_path.parent().expect("auth parent"))
        .expect("create codex dir");

    let mut store = CodexAccountStore::new(store_path);
    store
        .upsert_account(make_record("acct-old", true))
        .expect("seed existing active account");
    std::fs::write(
        &live_auth_path,
        serde_json::to_string_pretty(&sample_live_auth("watcher@example.com", "acct-new"))
            .expect("serialize auth"),
    )
    .expect("write live auth");

    let mut emitted = 0usize;
    let imported = sync_live_auth_once(&mut store, &live_auth_path, || {
        emitted += 1;
        Ok(())
    })
    .expect("sync live auth");

    assert_eq!(emitted, 1);
    assert!(imported.is_active);
    assert_eq!(imported.account_id.as_deref(), Some("acct-new"));
    assert_eq!(imported.email.as_deref(), Some("watcher@example.com"));

    let accounts = store.list_accounts().expect("list accounts");
    assert_eq!(accounts.len(), 2);
    assert!(accounts
        .iter()
        .find(|account| account.account_id.as_deref() == Some("acct-new"))
        .expect("new account")
        .is_active);
    assert!(!accounts
        .iter()
        .find(|account| account.account_id.as_deref() == Some("acct-old"))
        .expect("old account")
        .is_active);
}

#[test]
fn codex_auth_watcher_debounces_duplicate_burst_events() {
    let mut debouncer = AuthFileDebouncer::new(Duration::from_millis(100));
    let started = Instant::now();

    assert!(!debouncer.observe(11, started));
    assert!(!debouncer.observe(11, started + Duration::from_millis(50)));
    assert!(debouncer.observe(11, started + Duration::from_millis(125)));
    debouncer.mark_applied(11);

    assert!(!debouncer.observe(11, started + Duration::from_millis(150)));
    assert!(!debouncer.observe(22, started + Duration::from_millis(160)));
    assert!(!debouncer.observe(22, started + Duration::from_millis(240)));
    assert!(debouncer.observe(22, started + Duration::from_millis(280)));
}

#[test]
fn codex_accounts_watcher_poll_imports_live_auth_and_marks_account_active() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let live_auth_path = home.join(".codex").join("auth.json");
    std::fs::create_dir_all(live_auth_path.parent().expect("auth parent"))
        .expect("create codex dir");
    std::fs::write(
        &live_auth_path,
        serde_json::to_string_pretty(&sample_live_auth("watch@example.com", "acct-watch"))
            .expect("serialize auth"),
    )
    .expect("write live auth");

    let mut store = CodexAccountStore::new(home.join(".codex-switch").join("codex-accounts.json"));
    let mut previous_signature = None;

    let changed = poll_codex_auth_change(&mut store, &live_auth_path, &mut previous_signature)
        .expect("poll auth change");

    assert!(changed);
    let accounts = store.list_accounts().expect("list watched accounts");
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].email.as_deref(), Some("watch@example.com"));
    assert!(accounts[0].is_active);
}

#[test]
fn codex_accounts_watcher_poll_debounces_duplicate_content_bursts() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();

    let live_auth_path = home.join(".codex").join("auth.json");
    std::fs::create_dir_all(live_auth_path.parent().expect("auth parent"))
        .expect("create codex dir");
    std::fs::write(
        &live_auth_path,
        serde_json::to_string_pretty(&sample_live_auth("watch@example.com", "acct-watch"))
            .expect("serialize auth"),
    )
    .expect("write live auth");

    let mut store = CodexAccountStore::new(home.join(".codex-switch").join("codex-accounts.json"));
    let mut previous_signature = None;

    assert!(
        poll_codex_auth_change(&mut store, &live_auth_path, &mut previous_signature)
            .expect("first poll")
    );
    assert!(
        !poll_codex_auth_change(&mut store, &live_auth_path, &mut previous_signature)
            .expect("duplicate poll")
    );

    let accounts = store.list_accounts().expect("list watched accounts");
    assert_eq!(accounts.len(), 1);
}
