use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::error::AppError;
use crate::store::{AppState, CodexAccountsRuntime};

use super::{CodexAccountRecord, CodexAccountStore};

const CODEX_AUTH_WATCH_INTERVAL: Duration = Duration::from_millis(750);

static WATCHER_STARTED: OnceLock<()> = OnceLock::new();

pub trait LiveAuthSyncTarget {
    fn sync_from_live_auth(
        &mut self,
        live_auth_path: &Path,
    ) -> Result<CodexAccountRecord, AppError>;
}

impl LiveAuthSyncTarget for CodexAccountStore {
    fn sync_from_live_auth(
        &mut self,
        live_auth_path: &Path,
    ) -> Result<CodexAccountRecord, AppError> {
        self.bootstrap_from_live_auth(live_auth_path)
    }
}

impl LiveAuthSyncTarget for CodexAccountsRuntime {
    fn sync_from_live_auth(
        &mut self,
        live_auth_path: &Path,
    ) -> Result<CodexAccountRecord, AppError> {
        self.sync_from_live_auth(live_auth_path)
    }
}

pub struct AuthFileDebouncer {
    debounce_window: Duration,
    pending_hash: Option<u64>,
    pending_since: Option<Instant>,
    applied_hash: Option<u64>,
}

impl AuthFileDebouncer {
    pub fn new(debounce_window: Duration) -> Self {
        Self {
            debounce_window,
            pending_hash: None,
            pending_since: None,
            applied_hash: None,
        }
    }

    pub fn observe(&mut self, hash: u64, now: Instant) -> bool {
        if self.applied_hash == Some(hash) {
            self.pending_hash = None;
            self.pending_since = None;
            return false;
        }

        match self.pending_hash {
            None => {
                self.pending_hash = Some(hash);
                self.pending_since = Some(now);
                false
            }
            Some(current) if current != hash => {
                self.pending_hash = Some(hash);
                self.pending_since = Some(now);
                false
            }
            Some(_) => {
                let Some(started_at) = self.pending_since else {
                    self.pending_since = Some(now);
                    return false;
                };

                now.saturating_duration_since(started_at) >= self.debounce_window
            }
        }
    }

    pub fn mark_applied(&mut self, hash: u64) {
        self.applied_hash = Some(hash);
        self.pending_hash = None;
        self.pending_since = None;
    }
}

pub fn sync_live_auth_once<T, F>(
    target: &mut T,
    live_auth_path: &Path,
    mut emit_updated: F,
) -> Result<CodexAccountRecord, AppError>
where
    T: LiveAuthSyncTarget,
    F: FnMut() -> Result<(), AppError>,
{
    let imported = target.sync_from_live_auth(live_auth_path)?;
    emit_updated()?;
    Ok(imported)
}

pub fn poll_codex_auth_change(
    store: &mut CodexAccountStore,
    live_auth_path: &Path,
    previous_signature: &mut Option<u64>,
) -> Result<bool, AppError> {
    if !live_auth_path.exists() {
        return Ok(false);
    }

    let raw = std::fs::read(live_auth_path).map_err(|error| AppError::io(live_auth_path, error))?;
    let signature = content_signature(&raw);
    if previous_signature == &Some(signature) {
        return Ok(false);
    }

    store.bootstrap_from_live_auth(live_auth_path)?;
    *previous_signature = Some(signature);
    Ok(true)
}

pub fn spawn_codex_auth_watcher(app: AppHandle) {
    if WATCHER_STARTED.set(()).is_err() {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let live_auth_path = crate::get_codex_auth_path();
        let mut previous_signature = None;

        loop {
            tokio::time::sleep(CODEX_AUTH_WATCH_INTERVAL).await;

            let changed = {
                let state = app.state::<AppState>();
                let lock_result = state.codex_accounts.lock();
                match lock_result {
                    Ok(mut runtime) => match poll_codex_auth_change(
                        &mut runtime.store,
                        &live_auth_path,
                        &mut previous_signature,
                    ) {
                        Ok(changed) => changed,
                        Err(error) => {
                            log::warn!("Failed to poll Codex auth watcher: {error}");
                            false
                        }
                    },
                    Err(error) => {
                        log::warn!("Failed to lock Codex watcher state: {error}");
                        false
                    }
                }
            };

            if changed {
                crate::tray::refresh_tray_menu(&app);
                let _ = app.emit(
                    "codex-accounts-updated",
                    serde_json::json!({ "source": "auth.json" }),
                );
            }
        }
    });
}

fn content_signature(raw: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    raw.hash(&mut hasher);
    hasher.finish()
}
