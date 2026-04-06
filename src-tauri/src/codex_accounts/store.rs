use std::path::{Path, PathBuf};

use chrono::Utc;
use serde_json::Value;

use crate::config::get_app_config_dir;
use crate::error::AppError;

use super::auth_file::parse_codex_account_from_auth_json;
use super::{CodexAccountRecord, StoredCodexAccounts};

#[derive(Debug, thiserror::Error)]
pub enum CodexDeleteAccountError {
    #[error("cannot delete active Codex account")]
    ActiveAccount,
    #[error("Codex account not found")]
    NotFound,
    #[error(transparent)]
    App(#[from] AppError),
}

pub fn codex_accounts_store_path() -> PathBuf {
    get_app_config_dir().join("codex-accounts.json")
}

pub struct CodexAccountStore {
    path: PathBuf,
    stored: StoredCodexAccounts,
    load_error: Option<String>,
}

pub trait IntoUpsertRecord {
    fn into_upsert_record(self) -> Result<CodexAccountRecord, AppError>;
}

impl IntoUpsertRecord for CodexAccountRecord {
    fn into_upsert_record(self) -> Result<CodexAccountRecord, AppError> {
        Ok(self)
    }
}

impl IntoUpsertRecord for Value {
    fn into_upsert_record(self) -> Result<CodexAccountRecord, AppError> {
        parse_codex_account_from_auth_json(self).map(|mut account| {
            account.is_active = false;
            account.last_used_at = None;
            account
        })
    }
}

impl CodexAccountStore {
    pub fn new(path: PathBuf) -> Self {
        match StoredCodexAccounts::load(&path) {
            Ok(stored) => Self {
                path,
                stored,
                load_error: None,
            },
            Err(error) if path.exists() => Self {
                path,
                stored: StoredCodexAccounts::default(),
                load_error: Some(error.to_string()),
            },
            Err(_) => Self {
                path,
                stored: StoredCodexAccounts::default(),
                load_error: None,
            },
        }
    }

    pub fn load(&self) -> Result<StoredCodexAccounts, AppError> {
        self.ensure_store_ready()?;
        StoredCodexAccounts::load(&self.path)
    }

    pub fn save(&mut self, stored: &StoredCodexAccounts) -> Result<(), AppError> {
        self.ensure_store_ready()?;
        stored.save(&self.path)?;
        self.stored = StoredCodexAccounts::load(&self.path)?;
        Ok(())
    }

    pub fn list_accounts(&self) -> Result<Vec<CodexAccountRecord>, AppError> {
        self.ensure_store_ready()?;
        Ok(self.stored.accounts.clone())
    }

    pub fn upsert_account<T>(&mut self, input: T) -> Result<CodexAccountRecord, AppError>
    where
        T: IntoUpsertRecord,
    {
        let account = input.into_upsert_record()?;
        self.upsert_record(account)
    }

    fn upsert_record(
        &mut self,
        account: CodexAccountRecord,
    ) -> Result<CodexAccountRecord, AppError> {
        let lookup_key = record_lookup_key(&account);

        if let Some(index) = self
            .stored
            .accounts
            .iter()
            .position(|existing| same_account(existing, &lookup_key))
        {
            self.stored.accounts[index] = account.clone();
        } else {
            self.stored.accounts.push(account.clone());
        }

        if account.is_active {
            set_active_in_list(&mut self.stored.accounts, &lookup_key);
        } else {
            normalize_active_accounts(&mut self.stored.accounts);
        }

        self.persist()?;

        self.stored
            .accounts
            .iter()
            .find(|existing| same_account(existing, &lookup_key))
            .cloned()
            .ok_or_else(|| AppError::Message("saved Codex account missing after upsert".into()))
    }

    pub fn set_active_account(&mut self, account_id: &str) -> Result<(), AppError> {
        if !self
            .stored
            .accounts
            .iter()
            .any(|account| matches_target(account, account_id))
        {
            return Err(AppError::InvalidInput(format!(
                "Codex account not found: {account_id}"
            )));
        }

        set_active_in_list(&mut self.stored.accounts, account_id);
        self.persist()
    }

    pub fn delete_account(&mut self, account_id: &str) -> Result<(), CodexDeleteAccountError> {
        let index = self
            .stored
            .accounts
            .iter()
            .position(|account| matches_target(account, account_id))
            .ok_or(CodexDeleteAccountError::NotFound)?;

        if self.stored.accounts[index].is_active {
            return Err(CodexDeleteAccountError::ActiveAccount);
        }

        self.stored.accounts.remove(index);
        self.persist()?;
        Ok(())
    }

    pub fn bootstrap_from_live_auth(
        &mut self,
        live_auth_path: &Path,
    ) -> Result<CodexAccountRecord, AppError> {
        self.ensure_store_ready()?;
        let auth_json: Value = crate::config::read_json_file(live_auth_path)?;
        let mut imported = parse_codex_account_from_auth_json(auth_json)?;
        imported.is_active = true;
        if let Some(existing) = self
            .stored
            .accounts
            .iter()
            .find(|record| same_account(record, &record_lookup_key(&imported)))
        {
            imported.id = existing.id.clone();
            imported.added_at = existing.added_at;
            imported.last_used_at = existing.last_used_at;
            imported.quota = existing.quota.clone();
            imported.metadata = existing.metadata.clone();
        }
        self.upsert_record(imported)
    }

    fn persist(&mut self) -> Result<(), AppError> {
        self.ensure_store_ready()?;
        self.stored.save(&self.path)?;
        self.stored = StoredCodexAccounts::load(&self.path)?;
        Ok(())
    }

    fn ensure_store_ready(&self) -> Result<(), AppError> {
        if let Some(message) = &self.load_error {
            return Err(AppError::Config(format!(
                "Codex account store is unreadable: {message}"
            )));
        }

        Ok(())
    }
}

fn set_active_in_list(accounts: &mut [CodexAccountRecord], target: &str) {
    let now = now_timestamp();
    for account in accounts.iter_mut() {
        let is_target = matches_target(account, target);
        account.is_active = is_target;
        if is_target {
            account.last_used_at = Some(now);
        }
    }
}

fn normalize_active_accounts(accounts: &mut [CodexAccountRecord]) {
    let last_active = accounts.iter().rposition(|account| account.is_active);
    for (index, account) in accounts.iter_mut().enumerate() {
        account.is_active = Some(index) == last_active;
    }
}

fn record_lookup_key(record: &CodexAccountRecord) -> String {
    record
        .account_id
        .clone()
        .unwrap_or_else(|| record.id.clone())
}

fn same_account(record: &CodexAccountRecord, key: &str) -> bool {
    record.account_id.as_deref() == Some(key) || record.id == key
}

fn matches_target(record: &CodexAccountRecord, target: &str) -> bool {
    record.account_id.as_deref() == Some(target) || record.id == target
}

fn now_timestamp() -> i64 {
    Utc::now().timestamp()
}
