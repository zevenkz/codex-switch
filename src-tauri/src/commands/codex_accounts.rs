use tauri::{AppHandle, Emitter, State};

use futures::future::BoxFuture;
use std::process::Command;
use std::time::Duration;

use crate::codex_accounts::auth_file::{parse_codex_account_from_auth_json, write_live_codex_auth_json};
use crate::codex_accounts::oauth::build_authorize_url;
use crate::codex_accounts::oauth::OAuthCallbackOutcome;
use crate::codex_accounts::oauth::OAuthSession;
use crate::codex_accounts::{CodexAccountRecord, CodexDeleteAccountError, CodexQuotaSnapshot};
use crate::error::AppError;
use crate::store::AppState;

const DEFAULT_CODEX_OAUTH_CALLBACK_PORT: u16 = 1455;
const DEFAULT_CODEX_OAUTH_CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

fn notify_codex_accounts_changed(app: &AppHandle) {
    crate::tray::refresh_tray_menu(app);
    let _ = app.emit("codex-accounts-updated", serde_json::json!({ "source": "commands" }));
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingCodexAccountOAuthSession {
    pub state: String,
    pub authorize_url: String,
    pub callback_port: u16,
}

fn sync_live_auth(state: &AppState) -> Result<(), AppError> {
    let live_auth_path = crate::get_codex_auth_path();
    if !live_auth_path.exists() {
        return Ok(());
    }

    let mut runtime = state.codex_accounts.lock()?;
    runtime.store.bootstrap_from_live_auth(&live_auth_path)?;
    Ok(())
}

fn list_codex_accounts_impl(state: &AppState) -> Result<Vec<CodexAccountRecord>, AppError> {
    sync_live_auth(state)?;
    let runtime = state.codex_accounts.lock()?;
    runtime.store.list_accounts()
}

fn get_active_codex_account_impl(state: &AppState) -> Result<Option<CodexAccountRecord>, AppError> {
    Ok(list_codex_accounts_impl(state)?
        .into_iter()
        .find(|account| account.is_active))
}

fn start_codex_account_oauth_impl(
    state: &AppState,
) -> Result<PendingCodexAccountOAuthSession, AppError> {
    let session = OAuthSession::new(DEFAULT_CODEX_OAUTH_CALLBACK_PORT)?;
    let authorize_url = build_authorize_url(&session)?.to_string();
    let pending_session = PendingCodexAccountOAuthSession {
        state: session.state().to_string(),
        authorize_url,
        callback_port: session.callback_port(),
    };

    let mut runtime = state.codex_accounts.lock()?;
    runtime.pending_oauth_session = Some(session);

    Ok(pending_session)
}

fn cancel_codex_account_oauth_impl(state: &AppState) -> Result<bool, AppError> {
    let mut runtime = state.codex_accounts.lock()?;
    runtime.pending_oauth_session = None;
    Ok(true)
}

fn codex_account_lookup_key(record: &CodexAccountRecord) -> &str {
    record.account_id.as_deref().unwrap_or(&record.id)
}

fn matches_imported_oauth_account(
    record: &CodexAccountRecord,
    lookup_key: &str,
    imported_email: Option<&str>,
) -> bool {
    matches_codex_account(record, lookup_key)
        || imported_email
            .zip(record.email.as_deref())
            .is_some_and(|(imported_email, existing_email)| imported_email == existing_email)
}

fn merge_imported_oauth_account(
    mut imported: CodexAccountRecord,
    existing: Option<&CodexAccountRecord>,
) -> CodexAccountRecord {
    if let Some(existing) = existing {
        imported.id = existing.id.clone();
        imported.added_at = existing.added_at;
        imported.last_used_at = existing.last_used_at;
        imported.is_active = existing.is_active;
        imported.quota = existing.quota.clone();
        imported.metadata = existing.metadata.clone();
    } else {
        imported.is_active = false;
        imported.last_used_at = None;
    }

    imported
}

pub(crate) fn switch_codex_account_internal(
    state: &AppState,
    account_id: &str,
) -> Result<bool, AppError> {
    let target = {
        let runtime = state.codex_accounts.lock()?;
        runtime
            .store
            .list_accounts()?
            .into_iter()
            .find(|account| account.account_id.as_deref() == Some(account_id) || account.id == account_id)
            .ok_or_else(|| AppError::InvalidInput(format!("Codex account not found: {account_id}")))?
    };

    let mut live_target = target.clone();
    live_target.is_active = true;
    write_live_codex_auth_json(&crate::get_codex_auth_path(), &live_target)?;

    let mut runtime = state.codex_accounts.lock()?;
    runtime.store.set_active_account(account_id)?;
    Ok(true)
}

fn restart_codex_desktop_app() -> Result<(), AppError> {
    #[cfg(target_os = "macos")]
    {
        let quit_output = Command::new("osascript")
            .args([
                "-e",
                r#"if application "Codex" is running then tell application "Codex" to quit"#,
            ])
            .output()
            .map_err(|error| AppError::Message(format!("Failed to quit Codex: {error}")))?;
        if !quit_output.status.success() {
            return Err(AppError::Message(format!(
                "Failed to quit Codex: {}",
                String::from_utf8_lossy(&quit_output.stderr).trim()
            )));
        }

        std::thread::sleep(Duration::from_millis(600));

        let open_output = Command::new("open")
            .args(["-a", "Codex"])
            .output()
            .map_err(|error| AppError::Message(format!("Failed to launch Codex: {error}")))?;
        if !open_output.status.success() {
            return Err(AppError::Message(format!(
                "Failed to launch Codex: {}",
                String::from_utf8_lossy(&open_output.stderr).trim()
            )));
        }
    }

    Ok(())
}

pub(crate) fn switch_codex_account_and_restart_internal(
    state: &AppState,
    account_id: &str,
) -> Result<bool, AppError> {
    switch_codex_account_and_restart_with(state, account_id, restart_codex_desktop_app)
}

fn switch_codex_account_and_restart_with<R>(
    state: &AppState,
    account_id: &str,
    restart: R,
) -> Result<bool, AppError>
where
    R: FnOnce() -> Result<(), AppError>,
{
    switch_codex_account_internal(state, account_id)?;
    restart()?;
    Ok(true)
}

fn delete_codex_account_impl(state: &AppState, account_id: &str) -> Result<bool, AppError> {
    let mut runtime = state.codex_accounts.lock()?;
    match runtime.store.delete_account(account_id) {
        Ok(()) => Ok(true),
        Err(CodexDeleteAccountError::ActiveAccount) => {
            Err(AppError::Message("cannot delete active Codex account".to_string()))
        }
        Err(CodexDeleteAccountError::NotFound) => {
            Err(AppError::InvalidInput(format!("Codex account not found: {account_id}")))
        }
        Err(CodexDeleteAccountError::App(err)) => Err(err),
    }
}

fn matches_codex_account(record: &CodexAccountRecord, account_id: &str) -> bool {
    record.account_id.as_deref() == Some(account_id) || record.id == account_id
}

fn quota_error_snapshot(existing: Option<&CodexQuotaSnapshot>, error: String) -> CodexQuotaSnapshot {
    match existing {
        Some(snapshot) => {
            let mut snapshot = snapshot.clone();
            snapshot.last_error = Some(error);
            snapshot
        }
        None => CodexQuotaSnapshot {
            five_hour_percent: None,
            five_hour_reset_at: None,
            week_percent: None,
            week_reset_at: None,
            refreshed_at: chrono::Utc::now().timestamp_millis(),
            last_error: Some(error),
        },
    }
}

async fn complete_codex_account_oauth_with<L, E>(
    state: &AppState,
    listen_for_callback: L,
    exchange_code: E,
) -> Result<CodexAccountRecord, AppError>
where
    L: for<'a> FnOnce(
        &'a OAuthSession,
    ) -> BoxFuture<'a, Result<OAuthCallbackOutcome, AppError>>,
    E: for<'a> FnOnce(
        &'a OAuthSession,
        &'a str,
    ) -> BoxFuture<'a, Result<serde_json::Value, AppError>>,
{
    let session = {
        let mut runtime = state.codex_accounts.lock()?;
        runtime
            .pending_oauth_session
            .take()
            .ok_or_else(|| AppError::Message("No pending Codex OAuth session".to_string()))?
    };

    let callback = listen_for_callback(&session).await?;
    let code = match callback {
        OAuthCallbackOutcome::Authorized { code, .. } => code,
    };

    let auth_json = exchange_code(&session, &code).await?;
    let imported = parse_codex_account_from_auth_json(auth_json)?;
    let lookup_key = codex_account_lookup_key(&imported).to_string();
    let imported_email = imported.email.clone();

    let mut runtime = state.codex_accounts.lock()?;
    let existing = runtime
        .store
        .list_accounts()?
        .into_iter()
        .find(|record| {
            matches_imported_oauth_account(record, &lookup_key, imported_email.as_deref())
        });
    let merged = merge_imported_oauth_account(imported, existing.as_ref());
    let saved = runtime.store.upsert_account(merged)?;

    Ok(saved)
}

async fn refresh_codex_account_quota_with<F>(
    state: &AppState,
    account_id: &str,
    fetcher: F,
) -> Result<CodexAccountRecord, AppError>
where
    F: for<'a> Fn(&'a CodexAccountRecord) -> BoxFuture<'a, Result<CodexQuotaSnapshot, String>>,
{
    let account = {
        let runtime = state.codex_accounts.lock()?;
        runtime
            .store
            .list_accounts()?
            .into_iter()
            .find(|account| matches_codex_account(account, account_id))
            .ok_or_else(|| AppError::InvalidInput(format!("Codex account not found: {account_id}")))?
    };

    let (_, _, status, message) =
        crate::services::subscription::read_codex_credentials_from_auth_json_value(&account.auth_json);
    if !matches!(status, crate::services::subscription::CredentialStatus::Valid) {
        let message = message.unwrap_or_else(|| "Codex token has expired".to_string());
        let error_message = match status {
            crate::services::subscription::CredentialStatus::Expired => {
                format!("Codex token expired: {message}")
            }
            _ => message,
        };

        let mut runtime = state.codex_accounts.lock()?;
        let mut stored = runtime.store.load()?;
        if let Some(updated) = stored
            .accounts
            .iter_mut()
            .find(|record| matches_codex_account(record, account_id))
        {
            updated.quota = Some(quota_error_snapshot(updated.quota.as_ref(), error_message.clone()));
        }
        runtime.store.save(&stored)?;

        return Err(AppError::Message(error_message));
    }

    let snapshot = match fetcher(&account).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let mut runtime = state.codex_accounts.lock()?;
            let mut stored = runtime.store.load()?;
            if let Some(updated) = stored
                .accounts
                .iter_mut()
                .find(|record| matches_codex_account(record, account_id))
            {
                updated.quota = Some(quota_error_snapshot(updated.quota.as_ref(), error.clone()));
            }
            runtime.store.save(&stored)?;
            return Err(AppError::Message(error));
        }
    };
    let mut runtime = state.codex_accounts.lock()?;
    let mut stored = runtime.store.load()?;
    let saved = {
        let updated = stored
            .accounts
            .iter_mut()
            .find(|record| matches_codex_account(record, account_id))
            .ok_or_else(|| AppError::InvalidInput(format!("Codex account not found: {account_id}")))?;
        updated.quota = Some(CodexQuotaSnapshot {
            last_error: None,
            ..snapshot
        });
        updated.clone()
    };
    runtime.store.save(&stored)?;
    Ok(saved)
}

async fn refresh_all_codex_account_quotas_with<F>(
    state: &AppState,
    fetcher: F,
) -> Result<Vec<CodexAccountRecord>, AppError>
where
    F: for<'a> Fn(&'a CodexAccountRecord) -> BoxFuture<'a, Result<CodexQuotaSnapshot, String>>,
{
    let accounts = {
        let runtime = state.codex_accounts.lock()?;
        runtime.store.list_accounts()?
    };
    let mut stored = {
        let runtime = state.codex_accounts.lock()?;
        runtime.store.load()?
    };

    let mut refreshed = Vec::new();
    let mut errors = Vec::new();

    for account in accounts {
        let (_, _, status, message) =
            crate::services::subscription::read_codex_credentials_from_auth_json_value(
                &account.auth_json,
            );
        if !matches!(status, crate::services::subscription::CredentialStatus::Valid) {
            let message = message.unwrap_or_else(|| "Codex token has expired".to_string());
            let error_message = match status {
                crate::services::subscription::CredentialStatus::Expired => {
                    format!("Codex token expired: {message}")
                }
                _ => message,
            };
            if let Some(updated) = stored
                .accounts
                .iter_mut()
                .find(|record| {
                    matches_codex_account(
                        record,
                        account.account_id.as_deref().unwrap_or(&account.id),
                    )
                })
            {
                updated.quota = Some(quota_error_snapshot(
                    updated.quota.as_ref(),
                    error_message.clone(),
                ));
            }
            errors.push(error_message);
            continue;
        }

        match fetcher(&account).await {
            Ok(snapshot) => {
                if let Some(updated) = stored
                    .accounts
                    .iter_mut()
                    .find(|record| matches_codex_account(record, account.account_id.as_deref().unwrap_or(&account.id)))
                {
                    updated.quota = Some(CodexQuotaSnapshot {
                        last_error: None,
                        ..snapshot
                    });
                    refreshed.push(updated.clone());
                } else {
                    errors.push(format!(
                        "Codex account not found: {}",
                        account.account_id.as_deref().unwrap_or(&account.id)
                    ));
                }
            }
            Err(error) => {
                if let Some(updated) = stored
                    .accounts
                    .iter_mut()
                    .find(|record| {
                        matches_codex_account(
                            record,
                            account.account_id.as_deref().unwrap_or(&account.id),
                        )
                    })
                {
                    updated.quota = Some(quota_error_snapshot(updated.quota.as_ref(), error.clone()));
                }
                errors.push(error);
            }
        }
    }

    if !refreshed.is_empty() || !errors.is_empty() {
        let mut runtime = state.codex_accounts.lock()?;
        runtime.store.save(&stored)?;
    }

    if !errors.is_empty() {
        return Err(AppError::Message(errors.join("; ")));
    }

    Ok(refreshed)
}

async fn refresh_codex_account_quota_impl(
    state: &AppState,
    account_id: &str,
) -> Result<CodexAccountRecord, AppError> {
    refresh_codex_account_quota_with(state, account_id, |account| {
        Box::pin(async move {
            let quota =
                crate::services::subscription::get_saved_codex_subscription_quota(&account.auth_json)
                    .await?;
            crate::services::subscription::codex_quota_snapshot_from_subscription(&quota)
        })
    })
    .await
}

pub(crate) async fn refresh_all_codex_account_quotas_internal(
    state: &AppState,
) -> Result<Vec<CodexAccountRecord>, AppError> {
    refresh_all_codex_account_quotas_with(state, |account| {
        Box::pin(async move {
            let quota =
                crate::services::subscription::get_saved_codex_subscription_quota(&account.auth_json)
                    .await?;
            crate::services::subscription::codex_quota_snapshot_from_subscription(&quota)
        })
    })
    .await
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn list_codex_accounts_test_hook(
    state: &AppState,
) -> Result<Vec<CodexAccountRecord>, AppError> {
    list_codex_accounts_impl(state)
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn get_active_codex_account_test_hook(
    state: &AppState,
) -> Result<Option<CodexAccountRecord>, AppError> {
    get_active_codex_account_impl(state)
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn start_codex_account_oauth_test_hook(
    state: &AppState,
) -> Result<PendingCodexAccountOAuthSession, AppError> {
    start_codex_account_oauth_impl(state)
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn cancel_codex_account_oauth_test_hook(state: &AppState) -> Result<bool, AppError> {
    cancel_codex_account_oauth_impl(state)
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn complete_codex_account_oauth_test_hook<L, E>(
    state: &AppState,
    listen_for_callback: L,
    exchange_code: E,
) -> Result<CodexAccountRecord, AppError>
where
    L: for<'a> FnOnce(
        &'a OAuthSession,
    ) -> BoxFuture<'a, Result<OAuthCallbackOutcome, AppError>>,
    E: for<'a> FnOnce(
        &'a OAuthSession,
        &'a str,
    ) -> BoxFuture<'a, Result<serde_json::Value, AppError>>,
{
    complete_codex_account_oauth_with(state, listen_for_callback, exchange_code).await
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn switch_codex_account_test_hook(
    state: &AppState,
    account_id: String,
) -> Result<bool, AppError> {
    switch_codex_account_internal(state, &account_id)
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn switch_codex_account_and_restart_test_hook<R>(
    state: &AppState,
    account_id: String,
    restart: R,
) -> Result<bool, AppError>
where
    R: FnOnce() -> Result<(), AppError>,
{
    switch_codex_account_and_restart_with(state, &account_id, restart)
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn delete_codex_account_test_hook(
    state: &AppState,
    account_id: String,
) -> Result<bool, AppError> {
    delete_codex_account_impl(state, &account_id)
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn refresh_codex_account_quota_test_hook<F>(
    state: &AppState,
    account_id: String,
    fetcher: F,
) -> Result<CodexAccountRecord, AppError>
where
    F: for<'a> Fn(&'a CodexAccountRecord) -> BoxFuture<'a, Result<CodexQuotaSnapshot, String>>,
{
    refresh_codex_account_quota_with(state, &account_id, fetcher).await
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub async fn refresh_all_codex_account_quotas_test_hook<F>(
    state: &AppState,
    fetcher: F,
) -> Result<Vec<CodexAccountRecord>, AppError>
where
    F: for<'a> Fn(&'a CodexAccountRecord) -> BoxFuture<'a, Result<CodexQuotaSnapshot, String>>,
{
    refresh_all_codex_account_quotas_with(state, fetcher).await
}

#[tauri::command]
pub async fn list_codex_accounts(
    state: State<'_, AppState>,
) -> Result<Vec<CodexAccountRecord>, String> {
    list_codex_accounts_impl(&state).map_err(Into::into)
}

#[tauri::command]
pub async fn get_active_codex_account(
    state: State<'_, AppState>,
) -> Result<Option<CodexAccountRecord>, String> {
    get_active_codex_account_impl(&state).map_err(Into::into)
}

#[tauri::command]
pub async fn start_codex_account_oauth(
    state: State<'_, AppState>,
) -> Result<PendingCodexAccountOAuthSession, String> {
    start_codex_account_oauth_impl(&state).map_err(Into::into)
}

#[tauri::command]
pub async fn cancel_codex_account_oauth(state: State<'_, AppState>) -> Result<bool, String> {
    cancel_codex_account_oauth_impl(&state).map_err(Into::into)
}

#[tauri::command]
pub async fn complete_codex_account_oauth(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CodexAccountRecord, String> {
    let result = complete_codex_account_oauth_with(
        &state,
        |session| {
            Box::pin(async move {
                session
                    .listen_for_callback(DEFAULT_CODEX_OAUTH_CALLBACK_TIMEOUT)
                    .await
            })
        },
        |session, code| {
            Box::pin(async move {
                let client = reqwest::Client::new();
                session.exchange_code_for_tokens(&client, code).await
            })
        },
    )
    .await
    .map_err(Into::into);

    if result.is_ok() {
        notify_codex_accounts_changed(&app);
    }

    result
}

#[tauri::command]
pub async fn switch_codex_account(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
) -> Result<bool, String> {
    let result = switch_codex_account_and_restart_internal(&state, &account_id).map_err(Into::into);
    if matches!(result, Ok(true)) {
        notify_codex_accounts_changed(&app);
    }
    result
}

#[tauri::command]
pub async fn delete_codex_account(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
) -> Result<bool, String> {
    let result = delete_codex_account_impl(&state, &account_id).map_err(Into::into);
    if matches!(result, Ok(true)) {
        notify_codex_accounts_changed(&app);
    }
    result
}

#[tauri::command]
pub async fn refresh_codex_account_quota(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
) -> Result<CodexAccountRecord, String> {
    let result = refresh_codex_account_quota_impl(&state, &account_id)
        .await
        .map_err(Into::into);
    if result.is_ok() {
        notify_codex_accounts_changed(&app);
    }
    result
}

#[tauri::command]
pub async fn refresh_all_codex_account_quotas(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<CodexAccountRecord>, String> {
    let result = refresh_all_codex_account_quotas_internal(&state)
        .await
        .map_err(Into::into);
    if result.is_ok() {
        notify_codex_accounts_changed(&app);
    }
    result
}
