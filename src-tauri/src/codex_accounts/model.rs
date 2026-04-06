use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::{read_json_file, write_json_file};
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexQuotaSnapshot {
    pub five_hour_percent: Option<f64>,
    pub five_hour_reset_at: Option<i64>,
    pub week_percent: Option<f64>,
    pub week_reset_at: Option<i64>,
    pub refreshed_at: i64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexAccountRecord {
    pub id: String,
    pub email: Option<String>,
    pub account_id: Option<String>,
    pub plan_type: Option<String>,
    pub display_name: Option<String>,
    pub avatar_seed: String,
    pub added_at: i64,
    pub last_used_at: Option<i64>,
    pub is_active: bool,
    pub auth_json: serde_json::Value,
    pub quota: Option<CodexQuotaSnapshot>,
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoredCodexAccounts {
    #[serde(default)]
    pub accounts: Vec<CodexAccountRecord>,
}

impl StoredCodexAccounts {
    pub fn load(path: &Path) -> Result<Self, AppError> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let mut stored: Self = read_json_file(path)?;
        normalize_active_accounts(&mut stored.accounts);
        Ok(stored)
    }

    pub fn save(&self, path: &Path) -> Result<(), AppError> {
        let mut normalized = self.clone();
        normalize_active_accounts(&mut normalized.accounts);
        write_json_file(path, &normalized)
    }
}

fn normalize_active_accounts(accounts: &mut [CodexAccountRecord]) {
    let last_active = accounts.iter().rposition(|account| account.is_active);
    for (index, account) in accounts.iter_mut().enumerate() {
        account.is_active = Some(index) == last_active;
    }
}
