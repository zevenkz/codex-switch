use std::path::Path;

use base64::Engine;
use chrono::Utc;
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::config::write_json_file;
use crate::error::AppError;

use super::CodexAccountRecord;

pub fn parse_codex_account_from_auth_json(
    auth_json: Value,
) -> Result<CodexAccountRecord, AppError> {
    let token_claims = extract_token_claims(&auth_json);
    let account_id = extract_string(
        &auth_json,
        &token_claims,
        &[
            &["tokens", "account_id"],
            &["account_id"],
            &["user", "account_id"],
            &["account", "id"],
        ],
        &[
            &["https://api.openai.com/auth", "chatgpt_account_id"],
            &["account_id"],
            &["acct_id"],
            &["sub"],
        ],
    );

    if account_id.is_none() {
        return Err(AppError::Config(
            "Codex auth missing account_id".to_string(),
        ));
    }

    let email = extract_string(
        &auth_json,
        &token_claims,
        &[&["email"], &["profile", "email"], &["user", "email"]],
        &[&["email"]],
    );
    let plan_type = extract_string(
        &auth_json,
        &token_claims,
        &[&["plan_type"], &["plan"], &["subscription", "plan_type"]],
        &[
            &["https://api.openai.com/auth", "chatgpt_plan_type"],
            &["plan_type"],
            &["plan"],
        ],
    );
    let display_name = extract_string(
        &auth_json,
        &token_claims,
        &[
            &["display_name"],
            &["name"],
            &["profile", "display_name"],
            &["profile", "name"],
            &["user", "name"],
        ],
        &[&["name"], &["display_name"]],
    );

    let avatar_seed = extract_direct_string(&auth_json, &[&["avatar_seed"]]).unwrap_or_else(|| {
        account_id
            .clone()
            .or_else(|| email.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string())
    });
    let now = Utc::now().timestamp();

    Ok(CodexAccountRecord {
        id: account_id
            .clone()
            .unwrap_or_else(|| format!("codex-account-{}", Uuid::new_v4())),
        email,
        account_id,
        plan_type,
        display_name,
        avatar_seed,
        added_at: now,
        last_used_at: Some(now),
        is_active: true,
        auth_json,
        quota: None,
        metadata: Map::new(),
    })
}

pub fn write_live_codex_auth_json(
    path: &Path,
    account: &CodexAccountRecord,
) -> Result<(), AppError> {
    let auth_json = build_live_codex_auth_json(account);
    write_json_file(path, &auth_json)
}

fn build_live_codex_auth_json(account: &CodexAccountRecord) -> Value {
    let mut auth_json = match &account.auth_json {
        Value::Object(map) => Value::Object(map.clone()),
        _ => Value::Object(Map::new()),
    };

    ensure_object_path(&mut auth_json, &["tokens"]);
    normalize_top_level_oauth_tokens(&mut auth_json);

    if let Some(account_id) = &account.account_id {
        set_path(
            &mut auth_json,
            &["tokens", "account_id"],
            Value::String(account_id.clone()),
        );
    }
    if let Some(email) = &account.email {
        set_missing_or_null_path(
            &mut auth_json,
            &["profile", "email"],
            Value::String(email.clone()),
        );
    }
    if let Some(plan_type) = &account.plan_type {
        set_missing_or_null_path(
            &mut auth_json,
            &["plan_type"],
            Value::String(plan_type.clone()),
        );
    }
    if let Some(display_name) = &account.display_name {
        set_missing_or_null_path(
            &mut auth_json,
            &["display_name"],
            Value::String(display_name.clone()),
        );
    }
    set_missing_or_null_path(
        &mut auth_json,
        &["avatar_seed"],
        Value::String(account.avatar_seed.clone()),
    );
    set_missing_or_null_path(&mut auth_json, &["auth_mode"], Value::String("chatgpt".to_string()));
    set_missing_or_null_path(
        &mut auth_json,
        &["last_refresh"],
        Value::String(Utc::now().to_rfc3339()),
    );
    set_missing_or_null_path(&mut auth_json, &["OPENAI_API_KEY"], Value::Null);

    auth_json
}

fn normalize_top_level_oauth_tokens(root: &mut Value) {
    for field in ["access_token", "refresh_token", "id_token"] {
        let top_level = get_path(root, &[field]).cloned();
        if let Some(value) = top_level {
            set_path(root, &["tokens", field], value);
            remove_top_level_key(root, field);
        }
    }
}

fn extract_token_claims(auth_json: &Value) -> Vec<Value> {
    ["id_token", "access_token"]
        .into_iter()
        .filter_map(|token_name| {
            auth_json
                .get("tokens")
                .and_then(|tokens| tokens.get(token_name))
                .and_then(Value::as_str)
                .or_else(|| auth_json.get(token_name).and_then(Value::as_str))
                .and_then(decode_jwt_payload)
        })
        .collect()
}

fn decode_jwt_payload(token: &str) -> Option<Value> {
    let mut segments = token.split('.');
    let _header = segments.next()?;
    let payload = segments.next()?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn extract_string(
    auth_json: &Value,
    token_claims: &[Value],
    candidate_paths: &[&[&str]],
    claim_paths: &[&[&str]],
) -> Option<String> {
    extract_direct_string(auth_json, candidate_paths).or_else(|| {
        token_claims.iter().find_map(|claims| {
            extract_direct_string(claims, claim_paths)
        })
    })
}

fn extract_direct_string(value: &Value, candidate_paths: &[&[&str]]) -> Option<String> {
    candidate_paths.iter().find_map(|path| {
        let mut current = value;
        for key in *path {
            current = current.get(*key)?;
        }
        current.as_str().map(ToOwned::to_owned)
    })
}

fn ensure_object_path(value: &mut Value, path: &[&str]) {
    let mut current = value;
    for key in path {
        if !current.is_object() {
            *current = Value::Object(Map::new());
        }
        let obj = current.as_object_mut().expect("object after normalization");
        current = obj
            .entry((*key).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
}

fn set_path(root: &mut Value, path: &[&str], value: Value) {
    if path.is_empty() {
        *root = value;
        return;
    }

    let mut current = root;
    for key in &path[..path.len() - 1] {
        if !current.is_object() {
            *current = Value::Object(Map::new());
        }
        let obj = current.as_object_mut().expect("object after normalization");
        current = obj
            .entry((*key).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }

    if !current.is_object() {
        *current = Value::Object(Map::new());
    }
    let obj = current.as_object_mut().expect("object after normalization");
    obj.insert(path[path.len() - 1].to_string(), value);
}

fn set_missing_or_null_path(root: &mut Value, path: &[&str], value: Value) {
    if get_path(root, path).is_none_or(Value::is_null) {
        set_path(root, path, value);
    }
}

fn get_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn remove_top_level_key(value: &mut Value, key: &str) {
    if let Some(obj) = value.as_object_mut() {
        obj.remove(key);
    }
}
