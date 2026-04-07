use codex_switch_lib::codex_accounts::auth_file::parse_codex_account_from_auth_json;
use codex_switch_lib::codex_accounts::{codex_accounts_store_path, CodexAccountStore};
use codex_switch_lib::codex_accounts::CodexQuotaSnapshot;
use codex_switch_lib::codex_accounts::oauth::OAuthSession;
use codex_switch_lib::{
    cancel_codex_account_oauth_test_hook, delete_codex_account_test_hook,
    get_active_codex_account_test_hook, list_codex_accounts_test_hook,
    complete_codex_account_oauth_test_hook,
    quit_codex_applescript_for_test,
    refresh_all_codex_account_quotas_test_hook, refresh_codex_account_quota_test_hook,
    start_codex_account_oauth_test_hook, switch_codex_account_and_restart_test_hook,
    switch_codex_account_test_hook,
};

use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use codex_switch_lib::{AppState, Database};

fn ensure_test_home() -> &'static Path {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| {
        let base = std::env::temp_dir().join("codex-switch-test-home");
        if base.exists() {
            let _ = std::fs::remove_dir_all(&base);
        }
        std::fs::create_dir_all(&base).expect("create test home");
        std::env::set_var("CODEX_SWITCH_TEST_HOME", &base);
        std::env::set_var("HOME", &base);
        #[cfg(windows)]
        std::env::set_var("USERPROFILE", &base);
        base
    })
    .as_path()
}

fn reset_test_fs() {
    let home = ensure_test_home();
    for sub in [
        ".claude",
        ".codex",
        ".codex-switch",
        ".gemini",
        ".config",
        ".openclaw",
    ] {
        let path = home.join(sub);
        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
    let _ = codex_switch_lib::update_settings(codex_switch_lib::AppSettings::default());
}

fn test_mutex() -> &'static Mutex<()> {
    static MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    MUTEX.get_or_init(|| Mutex::new(()))
}

fn acquire_test_mutex() -> std::sync::MutexGuard<'static, ()> {
    test_mutex()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn create_test_state() -> Result<AppState, Box<dyn std::error::Error>> {
    let db = Arc::new(Database::init()?);
    Ok(AppState::new(db))
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
        "display_name": "Alex"
    })
}

fn sample_live_auth_with_marker(email: &str, account_id: &str, marker: &str) -> serde_json::Value {
    let mut auth = sample_live_auth(email, account_id);
    auth["custom"] = json!({ "marker": marker });
    auth
}

fn sample_expired_live_auth(email: &str, account_id: &str) -> serde_json::Value {
    let mut auth = sample_live_auth(email, account_id);
    auth["last_refresh"] = json!("2026-03-20T10:00:00Z");
    auth
}

fn sample_quota_snapshot(five_hour_percent: f64, week_percent: f64) -> CodexQuotaSnapshot {
    CodexQuotaSnapshot {
        five_hour_percent: Some(five_hour_percent),
        five_hour_reset_at: Some(111),
        week_percent: Some(week_percent),
        week_reset_at: Some(222),
        refreshed_at: 333,
        last_error: None,
    }
}

fn sample_oauth_exchange_auth_json(email: &str, account_id: &str) -> serde_json::Value {
    json!({
        "OPENAI_API_KEY": null,
        "auth_mode": "chatgpt",
        "last_refresh": "2026-04-06T10:00:00Z",
        "tokens": {
            "access_token": "access-token",
            "refresh_token": "refresh-token",
            "id_token": "id-token",
            "account_id": account_id
        },
        "profile": {
            "email": email
        },
        "plan_type": "plus",
        "display_name": "OAuth User",
        "avatar_seed": "oauth-seed"
    })
}

fn fake_jwt(payload: serde_json::Value) -> String {
    use base64::Engine;

    let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(r#"{"alg":"none","typ":"JWT"}"#);
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&payload).expect("serialize payload"));
    format!("{header}.{payload}.signature")
}

fn sample_oauth_exchange_token_response(email: &str, account_id: &str) -> serde_json::Value {
    json!({
        "access_token": fake_jwt(json!({
            "sub": account_id,
            "account_id": account_id,
            "email": email,
            "plan_type": "plus",
            "name": "OAuth User"
        })),
        "refresh_token": "refresh-token",
        "id_token": fake_jwt(json!({
            "sub": account_id,
            "email": email,
            "plan_type": "plus",
            "name": "OAuth User"
        })),
        "token_type": "Bearer",
        "expires_in": 3600
    })
}

fn seed_store_account(
    store_path: std::path::PathBuf,
    auth_json: serde_json::Value,
    is_active: bool,
) {
    let mut store = CodexAccountStore::new(store_path);
    let mut record = parse_codex_account_from_auth_json(auth_json).expect("parse account");
    record.is_active = is_active;
    store.upsert_account(record).expect("seed store account");
}

fn load_account(store_path: &std::path::PathBuf, account_id: &str) -> codex_switch_lib::codex_accounts::CodexAccountRecord {
    CodexAccountStore::new(store_path.clone())
        .list_accounts()
        .expect("list accounts")
        .into_iter()
        .find(|account| account.account_id.as_deref() == Some(account_id))
        .expect("account exists")
}

#[tokio::test]
async fn codex_accounts_commands_list_accounts_bootstraps_live_account_into_store() {
    let _guard = acquire_test_mutex();
    reset_test_fs();
    let home = ensure_test_home();

    let auth_path = home.join(".codex").join("auth.json");
    std::fs::create_dir_all(auth_path.parent().expect("auth parent")).expect("create codex dir");
    std::fs::write(
        &auth_path,
        serde_json::to_string_pretty(&sample_live_auth("active@example.com", "acct-live"))
            .expect("serialize auth"),
    )
    .expect("write auth");

    let state = create_test_state().expect("create test state");
    let accounts = list_codex_accounts_test_hook(&state)
        .await
        .expect("list accounts");

    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].email.as_deref(), Some("active@example.com"));
    assert!(accounts[0].is_active);

    let store = CodexAccountStore::new(codex_accounts_store_path());
    let persisted = store.list_accounts().expect("list persisted accounts");
    assert_eq!(persisted.len(), 1);
    assert!(persisted[0].is_active);
}

#[tokio::test]
async fn codex_accounts_commands_get_active_account_returns_active_record() {
    let _guard = acquire_test_mutex();
    reset_test_fs();
    let home = ensure_test_home();
    let store_path = home.join(".codex-switch").join("codex-accounts.json");
    let mut store = CodexAccountStore::new(store_path);
    let mut record =
        parse_codex_account_from_auth_json(sample_live_auth("active@example.com", "acct-1"))
            .expect("parse account");
    record.is_active = true;
    store.upsert_account(record).expect("save account");

    let state = create_test_state().expect("create test state");
    let active = get_active_codex_account_test_hook(&state)
        .await
        .expect("get active account")
        .expect("active account exists");

    assert_eq!(active.account_id.as_deref(), Some("acct-1"));
}

#[tokio::test]
async fn codex_accounts_commands_start_oauth_returns_pending_session() {
    let _guard = acquire_test_mutex();
    reset_test_fs();

    let state = create_test_state().expect("create test state");
    let session = start_codex_account_oauth_test_hook(&state)
        .await
        .expect("start oauth");
    let pending = state
        .codex_accounts
        .lock()
        .expect("lock codex runtime")
        .pending_oauth_session
        .clone()
        .expect("pending session saved");

    assert_eq!(pending.state(), session.state);
    assert_eq!(pending.callback_port(), session.callback_port);
    assert_eq!(pending.redirect_uri(), "http://localhost:1455/auth/callback");
    assert!(!session.state.is_empty());
    assert!(session
        .authorize_url
        .starts_with("https://auth.openai.com/oauth/authorize?"));
    assert_eq!(session.callback_port, 1455);
}

#[tokio::test]
async fn codex_accounts_commands_cancel_oauth_clears_pending_session() {
    let _guard = acquire_test_mutex();
    reset_test_fs();

    let state = create_test_state().expect("create test state");
    let started = start_codex_account_oauth_test_hook(&state)
        .await
        .expect("start oauth");
    assert!(!started.state.is_empty());

    cancel_codex_account_oauth_test_hook(&state)
        .await
        .expect("cancel oauth");

    let pending = state
        .codex_accounts
        .lock()
        .expect("lock codex runtime")
        .pending_oauth_session
        .as_ref()
        .map(|session| session.state().to_string());
    assert!(pending.is_none());
}

#[tokio::test]
async fn codex_accounts_commands_switch_account_rewrites_live_auth_and_marks_target_active() {
    let _guard = acquire_test_mutex();
    reset_test_fs();
    let home = ensure_test_home();

    let live_auth_path = home.join(".codex").join("auth.json");
    std::fs::create_dir_all(live_auth_path.parent().expect("auth parent"))
        .expect("create codex dir");
    std::fs::write(
        &live_auth_path,
        serde_json::to_string_pretty(&sample_live_auth("current@example.com", "acct-current"))
            .expect("serialize live auth"),
    )
    .expect("write live auth");

    let store_path = codex_accounts_store_path();
    seed_store_account(
        store_path.clone(),
        sample_live_auth("current@example.com", "acct-current"),
        true,
    );
    seed_store_account(
        store_path.clone(),
        sample_live_auth_with_marker("target@example.com", "acct-target", "switch-me"),
        false,
    );

    let state = create_test_state().expect("create test state");
    switch_codex_account_test_hook(&state, "acct-target".to_string())
        .await
        .expect("switch account");

    let written: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&live_auth_path).expect("read rewritten live auth"),
    )
    .expect("parse rewritten live auth");
    assert_eq!(
        written["tokens"]["account_id"],
        json!("acct-target"),
        "live auth should be rewritten from the target snapshot"
    );
    assert_eq!(
        written["custom"]["marker"],
        json!("switch-me"),
        "snapshot fields should be preserved"
    );

    let active = get_active_codex_account_test_hook(&state)
        .await
        .expect("get active account")
        .expect("active account exists");
    assert_eq!(active.account_id.as_deref(), Some("acct-target"));
}

#[tokio::test]
async fn codex_accounts_commands_switch_and_restart_invokes_restart_after_switching() {
    let _guard = acquire_test_mutex();
    reset_test_fs();
    let home = ensure_test_home();

    let live_auth_path = home.join(".codex").join("auth.json");
    std::fs::create_dir_all(live_auth_path.parent().expect("auth parent"))
        .expect("create codex dir");
    std::fs::write(
        &live_auth_path,
        serde_json::to_string_pretty(&sample_live_auth("current@example.com", "acct-current"))
            .expect("serialize live auth"),
    )
    .expect("write live auth");

    let store_path = codex_accounts_store_path();
    seed_store_account(
        store_path.clone(),
        sample_live_auth("current@example.com", "acct-current"),
        true,
    );
    seed_store_account(
        store_path.clone(),
        sample_live_auth_with_marker("target@example.com", "acct-target", "switch-me"),
        false,
    );

    let state = create_test_state().expect("create test state");
    let restarted = Arc::new(Mutex::new(false));
    let restarted_flag = Arc::clone(&restarted);

    switch_codex_account_and_restart_test_hook(&state, "acct-target".to_string(), move || {
        *restarted_flag.lock().expect("lock restart flag") = true;
        Ok(())
    })
    .await
    .expect("switch and restart");

    assert!(
        *restarted.lock().expect("lock restart flag"),
        "restart callback should be invoked after switching"
    );

    let active = get_active_codex_account_test_hook(&state)
        .await
        .expect("get active account")
        .expect("active account exists");
    assert_eq!(active.account_id.as_deref(), Some("acct-target"));
}

#[test]
fn codex_accounts_restart_script_waits_for_codex_to_exit_before_reopening() {
    let script = quit_codex_applescript_for_test();

    assert!(script.contains("tell application \"Codex\" to quit"));
    assert!(script.contains("exists process \"Codex\""));
    assert!(script.contains("Timed out waiting for Codex to quit"));
}

#[tokio::test]
async fn codex_accounts_commands_delete_account_rejects_active_and_allows_inactive() {
    let _guard = acquire_test_mutex();
    reset_test_fs();
    let home = ensure_test_home();

    let live_auth_path = home.join(".codex").join("auth.json");
    std::fs::create_dir_all(live_auth_path.parent().expect("auth parent"))
        .expect("create codex dir");
    std::fs::write(
        &live_auth_path,
        serde_json::to_string_pretty(&sample_live_auth("active@example.com", "acct-active"))
            .expect("serialize live auth"),
    )
    .expect("write live auth");

    let store_path = codex_accounts_store_path();
    seed_store_account(
        store_path.clone(),
        sample_live_auth("active@example.com", "acct-active"),
        true,
    );
    seed_store_account(
        store_path.clone(),
        sample_live_auth("inactive@example.com", "acct-inactive"),
        false,
    );

    let state = create_test_state().expect("create test state");

    let inactive_deleted = delete_codex_account_test_hook(&state, "acct-inactive".to_string())
        .await
        .expect("delete inactive account");
    assert!(inactive_deleted);

    let remaining = CodexAccountStore::new(store_path.clone())
        .list_accounts()
        .expect("list accounts after delete");
    assert_eq!(remaining.len(), 1);
    assert_eq!(
        remaining[0].account_id.as_deref(),
        Some("acct-active"),
        "active account should remain"
    );

    let err = delete_codex_account_test_hook(&state, "acct-active".to_string())
        .await
        .expect_err("deleting active account should fail");
    assert!(
        err.to_string().contains("active"),
        "error should mention active account, got: {}",
        err
    );
}

#[tokio::test]
async fn codex_accounts_commands_refresh_account_updates_saved_quota_snapshot() {
    let _guard = acquire_test_mutex();
    reset_test_fs();
    let home = ensure_test_home();

    let store_path = home.join(".codex-switch").join("codex-accounts.json");
    seed_store_account(
        store_path.clone(),
        sample_live_auth("quota@example.com", "acct-quota"),
        true,
    );

    let state = create_test_state().expect("create test state");
    let updated = refresh_codex_account_quota_test_hook(
        &state,
        "acct-quota".to_string(),
        |account| {
            assert_eq!(account.account_id.as_deref(), Some("acct-quota"));
            Box::pin(async { Ok(sample_quota_snapshot(37.5, 62.5)) })
        },
    )
    .await
    .expect("refresh account");

    assert_eq!(
        updated.quota.as_ref().and_then(|quota| quota.five_hour_percent),
        Some(37.5)
    );

    let stored = load_account(&store_path, "acct-quota");
    assert_eq!(
        stored.quota.as_ref().and_then(|quota| quota.five_hour_percent),
        Some(37.5)
    );
    assert_eq!(stored.quota.as_ref().and_then(|quota| quota.week_percent), Some(62.5));
}

#[tokio::test]
async fn codex_accounts_commands_refresh_all_preserves_old_quota_on_network_failure() {
    let _guard = acquire_test_mutex();
    reset_test_fs();
    let home = ensure_test_home();

    let store_path = home.join(".codex-switch").join("codex-accounts.json");
    seed_store_account(
        store_path.clone(),
        sample_live_auth("ok@example.com", "acct-ok"),
        false,
    );
    seed_store_account(
        store_path.clone(),
        sample_live_auth("down@example.com", "acct-down"),
        false,
    );

    let mut store = CodexAccountStore::new(store_path.clone());
    let mut stored = store.load().expect("load store");
    for account in &mut stored.accounts {
        account.quota = Some(sample_quota_snapshot(10.0, 20.0));
    }
    store.save(&stored).expect("seed quota snapshot");

    let state = create_test_state().expect("create test state");
    let result = refresh_all_codex_account_quotas_test_hook(&state, |account| {
        let account_id = account.account_id.clone();
        Box::pin(async move {
            match account_id.as_deref() {
                Some("acct-ok") => Ok(sample_quota_snapshot(80.0, 90.0)),
                Some("acct-down") => Err("network failure".to_string()),
                _ => Err("unexpected account".to_string()),
            }
        })
    })
    .await;

    assert!(result.is_err(), "batch refresh should surface the failure");

    let refreshed = load_account(&store_path, "acct-ok");
    assert_eq!(
        refreshed.quota.as_ref().and_then(|quota| quota.five_hour_percent),
        Some(80.0)
    );

    let preserved = load_account(&store_path, "acct-down");
    assert_eq!(
        preserved.quota.as_ref().and_then(|quota| quota.five_hour_percent),
        Some(10.0),
        "failed refresh should keep the previous quota snapshot"
    );
    assert_eq!(
        preserved.quota.as_ref().and_then(|quota| quota.last_error.as_deref()),
        Some("network failure")
    );
}

#[tokio::test]
async fn codex_accounts_commands_refresh_account_reports_expired_token_distinctly() {
    let _guard = acquire_test_mutex();
    reset_test_fs();
    let home = ensure_test_home();

    let store_path = home.join(".codex-switch").join("codex-accounts.json");
    seed_store_account(
        store_path.clone(),
        sample_expired_live_auth("expired@example.com", "acct-expired"),
        false,
    );

    let state = create_test_state().expect("create test state");
    let err = refresh_codex_account_quota_test_hook(&state, "acct-expired".to_string(), |_account| {
        Box::pin(async move { panic!("fetcher should not be called for expired tokens") })
    })
    .await
    .expect_err("expired token should fail refresh");

    assert!(
        err.to_string().to_lowercase().contains("expired"),
        "error should clearly report expiration, got: {}",
        err
    );

    let stored = load_account(&store_path, "acct-expired");
    assert_eq!(
        stored.quota.as_ref().and_then(|quota| quota.last_error.as_deref()),
        Some("Codex token expired: Codex token may be stale (>8 days since last refresh)")
    );
}

#[tokio::test]
async fn codex_accounts_commands_complete_oauth_saves_new_account_without_switching() {
    let _guard = acquire_test_mutex();
    reset_test_fs();

    let state = create_test_state().expect("create test state");
    start_codex_account_oauth_test_hook(&state)
        .await
        .expect("start oauth");

    let imported = complete_codex_account_oauth_test_hook(
        &state,
        |session: &OAuthSession| {
            let query = format!(
                "code=oauth-code-123&state={}",
                session.state()
            );
            Box::pin(async move {
                session.handle_callback_query(&query).await
            })
        },
        |_session: &OAuthSession, code: &str| {
            let code = code.to_string();
            Box::pin(async move {
                assert_eq!(code, "oauth-code-123");
                Ok(sample_oauth_exchange_auth_json(
                    "oauth@example.com",
                    "acct-oauth",
                ))
            })
        },
    )
    .await
    .expect("complete oauth");

    assert_eq!(imported.email.as_deref(), Some("oauth@example.com"));
    assert_eq!(imported.account_id.as_deref(), Some("acct-oauth"));
    assert!(
        !imported.is_active,
        "newly imported oauth account should not switch the current session"
    );

    let pending = state
        .codex_accounts
        .lock()
        .expect("lock codex runtime")
        .pending_oauth_session
        .as_ref()
        .map(|session| session.state().to_string());
    assert!(pending.is_none(), "pending session should be cleared after success");

    let accounts = list_codex_accounts_test_hook(&state)
        .await
        .expect("list accounts");
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].email.as_deref(), Some("oauth@example.com"));
    assert!(!accounts[0].is_active);
}

#[tokio::test]
async fn codex_accounts_commands_complete_oauth_rejects_state_mismatch_and_clears_pending_session() {
    let _guard = acquire_test_mutex();
    reset_test_fs();

    let state = create_test_state().expect("create test state");
    start_codex_account_oauth_test_hook(&state)
        .await
        .expect("start oauth");

    let err = complete_codex_account_oauth_test_hook(
        &state,
        |session: &OAuthSession| {
            Box::pin(async move {
                session
                    .handle_callback_query("code=oauth-code-123&state=wrong-state")
                    .await
            })
        },
        |_session: &OAuthSession, _code: &str| {
            Box::pin(async move { Ok(sample_oauth_exchange_auth_json("unused@example.com", "acct-unused")) })
        },
    )
    .await
    .expect_err("state mismatch should fail");

    assert!(
        err.to_string().to_lowercase().contains("state mismatch"),
        "error should mention state mismatch, got: {}",
        err
    );
    {
        let runtime = state
            .codex_accounts
            .lock()
            .expect("lock codex runtime");
        let pending = runtime.pending_oauth_session.as_ref();
        assert!(pending.is_none(), "failed completion should clear pending session");
    }
    let accounts = list_codex_accounts_test_hook(&state)
        .await
        .expect("list accounts");
    assert!(accounts.is_empty(), "mismatched callback should not create an account");
}

#[tokio::test]
async fn codex_accounts_commands_complete_oauth_reports_missing_pending_session() {
    let _guard = acquire_test_mutex();
    reset_test_fs();

    let state = create_test_state().expect("create test state");

    let err = complete_codex_account_oauth_test_hook(
        &state,
        |_session: &OAuthSession| {
            Box::pin(async move {
                panic!("callback listener should not be called without a pending session")
            })
        },
        |_session: &OAuthSession, _code: &str| {
            Box::pin(async move { panic!("token exchange should not be called without a pending session") })
        },
    )
    .await
    .expect_err("missing pending session should fail");

    assert!(
        err.to_string().to_lowercase().contains("pending"),
        "error should mention pending session, got: {}",
        err
    );
}

#[tokio::test]
async fn codex_accounts_commands_complete_oauth_surfaces_token_exchange_failure() {
    let _guard = acquire_test_mutex();
    reset_test_fs();

    let state = create_test_state().expect("create test state");
    start_codex_account_oauth_test_hook(&state)
        .await
        .expect("start oauth");

    let err = complete_codex_account_oauth_test_hook(
        &state,
        |session: &OAuthSession| {
            let query = format!(
                "code=oauth-code-123&state={}",
                session.state()
            );
            Box::pin(async move {
                session.handle_callback_query(&query).await
            })
        },
        |_session: &OAuthSession, _code: &str| {
            Box::pin(async move { Err(codex_switch_lib::AppError::Message("token exchange failed".to_string())) })
        },
    )
    .await
    .expect_err("token exchange failure should fail");

    assert!(
        err.to_string().contains("token exchange failed"),
        "error should surface the exchange failure, got: {}",
        err
    );
    {
        let runtime = state
            .codex_accounts
            .lock()
            .expect("lock codex runtime");
        let pending = runtime.pending_oauth_session.as_ref();
        assert!(pending.is_none(), "failed completion should clear pending session");
    }
    let accounts = list_codex_accounts_test_hook(&state)
        .await
        .expect("list accounts");
    assert!(accounts.is_empty(), "failed exchange should not persist an account");
}

#[tokio::test]
async fn codex_accounts_commands_complete_oauth_accepts_top_level_token_response_shape() {
    let _guard = acquire_test_mutex();
    reset_test_fs();

    let state = create_test_state().expect("create test state");
    start_codex_account_oauth_test_hook(&state)
        .await
        .expect("start oauth");

    let imported = complete_codex_account_oauth_test_hook(
        &state,
        |session: &OAuthSession| {
            let query = format!("code=oauth-code-456&state={}", session.state());
            Box::pin(async move { session.handle_callback_query(&query).await })
        },
        |_session: &OAuthSession, code: &str| {
            let code = code.to_string();
            Box::pin(async move {
                assert_eq!(code, "oauth-code-456");
                Ok(sample_oauth_exchange_token_response(
                    "top-level@example.com",
                    "acct-top-level",
                ))
            })
        },
    )
    .await
    .expect("complete oauth from token response");

    assert_eq!(imported.email.as_deref(), Some("top-level@example.com"));
    assert_eq!(imported.account_id.as_deref(), Some("acct-top-level"));
    assert!(!imported.is_active);
}

#[tokio::test]
async fn codex_accounts_commands_refresh_account_accepts_top_level_oauth_token_shape() {
    let _guard = acquire_test_mutex();
    reset_test_fs();
    let home = ensure_test_home();

    let store_path = home.join(".codex-switch").join("codex-accounts.json");
    seed_store_account(
        store_path.clone(),
        sample_oauth_exchange_token_response("top-level@example.com", "acct-top-level"),
        false,
    );

    let state = create_test_state().expect("create test state");
    let refreshed = refresh_codex_account_quota_test_hook(
        &state,
        "acct-top-level".to_string(),
        |_account| Box::pin(async move { Ok(sample_quota_snapshot(41.0, 59.0)) }),
    )
    .await
    .expect("top-level token shape should be treated as oauth");

    assert_eq!(
        refreshed.quota.as_ref().and_then(|quota| quota.five_hour_percent),
        Some(41.0)
    );
    assert_eq!(
        refreshed.quota.as_ref().and_then(|quota| quota.week_percent),
        Some(59.0)
    );
}
