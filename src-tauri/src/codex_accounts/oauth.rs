use base64::Engine;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use url::Url;

use crate::error::AppError;

const OPENAI_AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
pub const OPENAI_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const OPENAI_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const OPENAI_SCOPE: &str = "openid profile email offline_access";
const OPENAI_ORIGINATOR: &str = "codex_vscode";

#[derive(Debug, Clone)]
pub struct PkcePair {
    pub code_verifier: String,
    pub code_challenge: String,
}

#[derive(Debug, Clone)]
pub struct OAuthSession {
    state: String,
    code_verifier: String,
    callback_port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuthCallbackOutcome {
    Authorized { code: String, state: String },
}

pub type OAuthTokenPayload = serde_json::Value;

impl OAuthSession {
    pub fn new(callback_port: u16) -> Result<Self, AppError> {
        let pkce = generate_pkce_pair()?;
        Ok(Self {
            state: uuid::Uuid::new_v4().simple().to_string(),
            code_verifier: pkce.code_verifier,
            callback_port,
        })
    }

    pub fn new_for_test(state: String, code_verifier: String, callback_port: u16) -> Self {
        Self {
            state,
            code_verifier,
            callback_port,
        }
    }

    pub fn state(&self) -> &str {
        &self.state
    }

    pub fn callback_port(&self) -> u16 {
        self.callback_port
    }

    pub fn code_verifier(&self) -> &str {
        &self.code_verifier
    }

    pub fn code_challenge(&self) -> String {
        let digest = Sha256::digest(self.code_verifier.as_bytes());
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://localhost:{}/auth/callback", self.callback_port)
    }

    pub async fn handle_callback_query(
        &self,
        raw_query: &str,
    ) -> Result<OAuthCallbackOutcome, AppError> {
        let params: std::collections::HashMap<String, String> =
            url::form_urlencoded::parse(raw_query.as_bytes())
                .into_owned()
                .collect();

        if let Some(error) = params.get("error") {
            let description = params
                .get("error_description")
                .cloned()
                .unwrap_or_else(|| "OAuth callback returned an error".to_string());
            return Err(AppError::Message(format!("{error}: {description}")));
        }

        let state = params
            .get("state")
            .ok_or_else(|| AppError::InvalidInput("OAuth callback missing state".to_string()))?;
        if state != &self.state {
            return Err(AppError::InvalidInput(
                "OAuth callback state mismatch".to_string(),
            ));
        }

        let code = params
            .get("code")
            .cloned()
            .ok_or_else(|| AppError::InvalidInput("OAuth callback missing code".to_string()))?;

        Ok(OAuthCallbackOutcome::Authorized {
            code,
            state: state.clone(),
        })
    }

    pub async fn listen_for_callback(
        &self,
        timeout: Duration,
    ) -> Result<OAuthCallbackOutcome, AppError> {
        self.listen_for_callback_with_cancel(timeout, std::future::pending::<()>())
            .await
    }

    pub async fn listen_for_callback_with_cancel<F>(
        &self,
        timeout: Duration,
        cancel: F,
    ) -> Result<OAuthCallbackOutcome, AppError>
    where
        F: std::future::Future<Output = ()>,
    {
        let started_at = Instant::now();
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", self.callback_port))
            .await
            .map_err(|error| {
                AppError::Message(format!(
                    "Failed to bind OAuth callback port {}: {error}",
                    self.callback_port
                ))
            })?;
        tokio::pin!(cancel);

        loop {
            let remaining = timeout
                .checked_sub(started_at.elapsed())
                .ok_or_else(|| {
                    AppError::Message("Timed out waiting for OAuth callback".to_string())
                })?;

            let (mut stream, _) = tokio::select! {
                _ = &mut cancel => {
                    return Err(AppError::Message("OAuth callback listener cancelled".to_string()));
                }
                accept = tokio::time::timeout(remaining, listener.accept()) => {
                    accept
                        .map_err(|_| AppError::Message("Timed out waiting for OAuth callback".to_string()))?
                        .map_err(|error| AppError::Message(format!("Failed to accept OAuth callback: {error}")))?
                }
            };

            let remaining = timeout
                .checked_sub(started_at.elapsed())
                .unwrap_or_else(|| Duration::from_millis(1));
            let outcome = match read_callback_query_with_timeout(&mut stream, remaining).await {
                Ok(query) => self.handle_callback_query(&query).await,
                Err(error) => Err(error),
            };

            let response = match &outcome {
                Ok(_) => success_http_response(),
                Err(error) => error_http_response(&error.to_string()),
            };
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;

            match outcome {
                Ok(outcome) => return Ok(outcome),
                Err(error) if should_continue_waiting_for_callback(&error) => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub async fn exchange_code_for_tokens(
        &self,
        client: &reqwest::Client,
        code: &str,
    ) -> Result<OAuthTokenPayload, AppError> {
        let response = client
            .post(OPENAI_TOKEN_URL)
            .form(&build_token_exchange_form(self, code))
            .send()
            .await
            .map_err(|error| AppError::Message(format!("OAuth token exchange failed: {error}")))?;

        let status = response.status();
        let body = response.text().await.map_err(|error| {
            AppError::Message(format!("Failed to read OAuth token response body: {error}"))
        })?;

        if !status.is_success() {
            return Err(AppError::Message(format!(
                "OAuth token exchange returned {}: {}",
                status, body
            )));
        }

        serde_json::from_str(&body).map_err(|error| {
            AppError::Message(format!(
                "Failed to parse OAuth token response JSON: {error}"
            ))
        })
    }
}

pub fn generate_pkce_pair() -> Result<PkcePair, AppError> {
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);

    Ok(PkcePair {
        code_verifier: verifier,
        code_challenge: challenge,
    })
}

pub fn build_authorize_url(session: &OAuthSession) -> Result<Url, AppError> {
    let mut url = Url::parse(OPENAI_AUTHORIZE_URL)
        .map_err(|error| AppError::Message(format!("Invalid OpenAI authorize URL: {error}")))?;

    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", OPENAI_CLIENT_ID)
        .append_pair("redirect_uri", &session.redirect_uri())
        .append_pair("scope", OPENAI_SCOPE)
        .append_pair("state", session.state())
        .append_pair("code_challenge", &session.code_challenge())
        .append_pair("code_challenge_method", "S256")
        .append_pair("id_token_add_organizations", "true")
        .append_pair("codex_cli_simplified_flow", "true")
        .append_pair("originator", OPENAI_ORIGINATOR);

    Ok(url)
}

pub fn build_token_exchange_form(session: &OAuthSession, code: &str) -> Vec<(String, String)> {
    vec![
        ("grant_type".to_string(), "authorization_code".to_string()),
        ("client_id".to_string(), OPENAI_CLIENT_ID.to_string()),
        ("code".to_string(), code.to_string()),
        ("redirect_uri".to_string(), session.redirect_uri()),
        (
            "code_verifier".to_string(),
            session.code_verifier().to_string(),
        ),
    ]
}

pub async fn await_callback_signal<T, F, C>(
    accept: F,
    timeout: Duration,
    cancel: C,
) -> Result<T, AppError>
where
    F: std::future::Future<Output = Result<T, AppError>>,
    C: std::future::Future<Output = ()>,
{
    tokio::pin!(accept);
    tokio::pin!(cancel);

    tokio::select! {
        _ = &mut cancel => Err(AppError::Message("OAuth callback listener cancelled".to_string())),
        result = tokio::time::timeout(timeout, &mut accept) => {
            result.map_err(|_| AppError::Message("Timed out waiting for OAuth callback".to_string()))?
        }
    }
}

fn extract_callback_query(request: &str) -> Result<String, AppError> {
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| AppError::InvalidInput("OAuth callback request was empty".to_string()))?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| AppError::InvalidInput("OAuth callback request missing path".to_string()))?;
    let url = Url::parse(&format!("http://localhost{path}"))
        .map_err(|error| AppError::InvalidInput(format!("Invalid OAuth callback path: {error}")))?;
    if url.path() != "/auth/callback" {
        return Err(AppError::InvalidInput(format!(
            "Unexpected OAuth callback path: {}",
            url.path()
        )));
    }
    Ok(url.query().unwrap_or_default().to_string())
}

pub async fn read_callback_query_with_timeout<R>(
    stream: &mut R,
    timeout: Duration,
) -> Result<String, AppError>
where
    R: AsyncRead + Unpin,
{
    let request = tokio::time::timeout(timeout, read_http_request(stream))
        .await
        .map_err(|_| AppError::Message("Timed out waiting for OAuth callback body".to_string()))??;

    extract_callback_query(&request)
}

pub async fn read_http_request<R>(stream: &mut R) -> Result<String, AppError>
where
    R: AsyncRead + Unpin,
{
    const MAX_REQUEST_BYTES: usize = 16 * 1024;

    let mut buffer = Vec::with_capacity(1024);
    loop {
        let mut chunk = [0u8; 1024];
        let bytes_read = stream.read(&mut chunk).await.map_err(|error| {
            AppError::Message(format!("Failed to read OAuth callback: {error}"))
        })?;

        if bytes_read == 0 {
            break;
        }

        buffer.extend_from_slice(&chunk[..bytes_read]);

        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }

        if buffer.len() >= MAX_REQUEST_BYTES {
            return Err(AppError::InvalidInput(
                "OAuth callback request headers exceeded maximum size".to_string(),
            ));
        }
    }

    if buffer.is_empty() {
        return Err(AppError::InvalidInput(
            "OAuth callback request was empty".to_string(),
        ));
    }

    Ok(String::from_utf8_lossy(&buffer).to_string())
}

fn success_http_response() -> String {
    "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\nconnection: close\r\n\r\n<!doctype html><html><body><h1>Codex Switch</h1><p>Authentication complete. This window will close automatically.</p><script>window.open('','_self');window.close();</script></body></html>".to_string()
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub fn success_http_response_for_test() -> String {
    success_http_response()
}

fn error_http_response(message: &str) -> String {
    format!(
        "HTTP/1.1 400 Bad Request\r\ncontent-type: text/html; charset=utf-8\r\nconnection: close\r\n\r\n<html><body><h1>Codex Switch</h1><p>{}</p></body></html>",
        html_escape(message)
    )
}

fn should_continue_waiting_for_callback(error: &AppError) -> bool {
    matches!(error, AppError::InvalidInput(_))
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
