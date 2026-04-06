use crate::database::Database;
use crate::codex_accounts::{oauth::OAuthSession, CodexAccountStore};
use crate::error::AppError;
use crate::get_codex_auth_path;
use crate::services::ProxyService;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct CodexAccountsRuntime {
    pub store: CodexAccountStore,
    pub pending_oauth_session: Option<OAuthSession>,
}

impl CodexAccountsRuntime {
    fn new() -> Self {
        let mut store = CodexAccountStore::new(crate::codex_accounts::codex_accounts_store_path());
        let live_auth_path = get_codex_auth_path();
        if live_auth_path.exists() {
            if let Err(error) = store.bootstrap_from_live_auth(&live_auth_path) {
                log::warn!("Failed to bootstrap Codex account store from live auth: {error}");
            }
        }

        Self {
            store,
            pending_oauth_session: None,
        }
    }

    pub fn sync_from_live_auth(
        &mut self,
        live_auth_path: &Path,
    ) -> Result<crate::codex_accounts::CodexAccountRecord, AppError> {
        self.store.bootstrap_from_live_auth(live_auth_path)
    }
}

/// 全局应用状态
pub struct AppState {
    pub db: Arc<Database>,
    pub proxy_service: ProxyService,
    pub codex_accounts: Mutex<CodexAccountsRuntime>,
}

impl AppState {
    /// 创建新的应用状态
    pub fn new(db: Arc<Database>) -> Self {
        let proxy_service = ProxyService::new(db.clone());
        let codex_accounts = Mutex::new(CodexAccountsRuntime::new());

        Self {
            db,
            proxy_service,
            codex_accounts,
        }
    }
}
