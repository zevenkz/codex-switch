use cc_switch_lib::codex_accounts::oauth::{
    await_callback_signal, build_authorize_url, build_token_exchange_form, generate_pkce_pair,
    read_callback_query_with_timeout, read_http_request, success_http_response_for_test,
    OAuthCallbackOutcome, OAuthSession,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn codex_accounts_oauth_callback_rejects_mismatched_state() {
    let session =
        OAuthSession::new_for_test("expected-state".to_string(), "verifier".to_string(), 1455);

    let result = session.handle_callback_query("code=abc&state=wrong").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn codex_accounts_oauth_callback_accepts_matching_state_and_code() {
    let session =
        OAuthSession::new_for_test("expected-state".to_string(), "verifier".to_string(), 1455);

    let result = session
        .handle_callback_query("code=abc123&state=expected-state")
        .await
        .expect("callback should succeed");

    assert!(matches!(
        result,
        OAuthCallbackOutcome::Authorized { code, .. } if code == "abc123"
    ));
}

#[test]
fn codex_accounts_oauth_build_authorize_url_contains_expected_parameters() {
    let session =
        OAuthSession::new_for_test("state-123".to_string(), "verifier-123".to_string(), 1455);

    let url = build_authorize_url(&session).expect("build authorize url");
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();

    assert_eq!(
        url.as_str().split('?').next(),
        Some("https://auth.openai.com/oauth/authorize")
    );
    assert_eq!(query.get("response_type").map(String::as_str), Some("code"));
    assert_eq!(
        query.get("client_id").map(String::as_str),
        Some("app_EMoamEEZ73f0CkXaXp7hrann")
    );
    assert_eq!(
        query.get("redirect_uri").map(String::as_str),
        Some("http://localhost:1455/auth/callback")
    );
    assert_eq!(query.get("state").map(String::as_str), Some("state-123"));
    assert_eq!(
        query.get("scope").map(String::as_str),
        Some("openid profile email offline_access")
    );
    assert_eq!(
        query.get("code_challenge_method").map(String::as_str),
        Some("S256")
    );
    assert_eq!(
        query.get("id_token_add_organizations").map(String::as_str),
        Some("true")
    );
    assert_eq!(
        query.get("codex_cli_simplified_flow").map(String::as_str),
        Some("true")
    );
    assert_eq!(
        query.get("originator").map(String::as_str),
        Some("codex_vscode")
    );
    assert!(query.contains_key("code_challenge"));
}

#[test]
fn codex_accounts_oauth_generate_pkce_pair_returns_url_safe_values() {
    let pair = generate_pkce_pair().expect("generate pkce pair");

    assert!(!pair.code_verifier.is_empty());
    assert!(!pair.code_challenge.is_empty());
    assert!(!pair.code_verifier.contains('='));
    assert!(!pair.code_challenge.contains('='));
    assert_ne!(pair.code_verifier, pair.code_challenge);
}

#[test]
fn codex_accounts_oauth_session_new_generates_state_and_redirect_uri() {
    let session = OAuthSession::new(1455).expect("create oauth session");

    assert!(!session.state().is_empty());
    assert!(!session.code_verifier().is_empty());
    assert_eq!(session.callback_port(), 1455);
    assert_eq!(
        session.redirect_uri(),
        "http://localhost:1455/auth/callback"
    );
}

#[test]
fn codex_accounts_oauth_token_exchange_form_contains_expected_fields() {
    let session =
        OAuthSession::new_for_test("state-123".to_string(), "verifier-123".to_string(), 1455);

    let form = build_token_exchange_form(&session, "code-123");

    let as_map: std::collections::HashMap<_, _> = form.into_iter().collect();
    assert_eq!(
        as_map.get("grant_type").map(String::as_str),
        Some("authorization_code")
    );
    assert_eq!(
        as_map.get("client_id").map(String::as_str),
        Some("app_EMoamEEZ73f0CkXaXp7hrann")
    );
    assert_eq!(as_map.get("code").map(String::as_str), Some("code-123"));
    assert_eq!(
        as_map.get("redirect_uri").map(String::as_str),
        Some("http://localhost:1455/auth/callback")
    );
    assert_eq!(
        as_map.get("code_verifier").map(String::as_str),
        Some("verifier-123")
    );
}

#[tokio::test]
async fn codex_accounts_oauth_listener_times_out_without_callback() {
    let result = await_callback_signal(
        async {
            std::future::pending::<()>().await;
            Ok::<(), cc_switch_lib::AppError>(())
        },
        std::time::Duration::from_millis(25),
        std::future::pending::<()>(),
    )
    .await;

    assert!(result.is_err());
    assert!(result
        .expect_err("listener should timeout")
        .to_string()
        .contains("Timed out"));
}

#[tokio::test]
async fn codex_accounts_oauth_listener_can_be_cancelled() {
    let result = await_callback_signal(
        async {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            Ok::<(), cc_switch_lib::AppError>(())
        },
        std::time::Duration::from_secs(5),
        async {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        },
    )
    .await;

    assert!(result.is_err());
    assert!(result
        .expect_err("listener should cancel")
        .to_string()
        .contains("cancelled"));
}

#[tokio::test]
async fn codex_accounts_oauth_listener_times_out_after_accepting_incomplete_request() {
    let (mut reader, mut writer) = tokio::io::duplex(256);
    let writer_task = tokio::spawn(async move {
        writer
            .write_all(
                b"GET /auth/callback?code=abc123&state=expected-state HTTP/1.1\r\nHost: localhost\r\n",
            )
            .await
            .expect("write partial request");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    });

    let result =
        read_callback_query_with_timeout(&mut reader, std::time::Duration::from_millis(25)).await;

    writer_task.await.expect("partial writer task");
    assert!(result.is_err());
    assert!(
        result
            .expect_err("listener should timeout after accept")
            .to_string()
            .contains("Timed out waiting for OAuth callback body")
    );
}

#[tokio::test]
async fn codex_accounts_oauth_listener_accepts_callback_split_across_tcp_chunks() {
    let (mut reader, mut writer) = tokio::io::duplex(256);
    let writer_task = tokio::spawn(async move {
        writer
            .write_all(b"GET /auth/callback?code=abc123&state=expected-state HTTP/1.1\r\n")
            .await
            .expect("write request line");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        writer
            .write_all(b"Host: localhost\r\n")
            .await
            .expect("write host header");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        writer
            .write_all(b"\r\n")
            .await
            .expect("terminate headers");
    });

    let request = read_http_request(&mut reader)
        .await
        .expect("read split request");
    let session =
        OAuthSession::new_for_test("expected-state".to_string(), "verifier".to_string(), 1455);
    let query = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|path| path.split_once('?').map(|(_, query)| query.to_string()))
        .expect("extract query from request");

    writer_task.await.expect("split writer task");
    let result = session
        .handle_callback_query(&query)
        .await
        .expect("handle split callback");

    assert!(matches!(
        result,
        OAuthCallbackOutcome::Authorized { code, state } if code == "abc123" && state == "expected-state"
    ));
}

#[test]
fn codex_accounts_oauth_success_page_attempts_to_close_the_window() {
    let response = success_http_response_for_test();

    assert!(response.contains("window.close()"));
    assert!(response.contains("Authentication complete"));
}

#[tokio::test]
async fn codex_accounts_oauth_listener_ignores_stale_state_mismatch_until_valid_callback_arrives() {
    let callback_port = {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind temp port");
        listener.local_addr().expect("temp addr").port()
    };
    let session = OAuthSession::new_for_test(
        "expected-state".to_string(),
        "verifier".to_string(),
        callback_port,
    );

    let listener_task = tokio::spawn({
        let session = session.clone();
        async move {
            session
                .listen_for_callback(std::time::Duration::from_millis(750))
                .await
        }
    });

    tokio::time::sleep(std::time::Duration::from_millis(25)).await;

    let mut stale_stream = tokio::net::TcpStream::connect(("127.0.0.1", callback_port))
        .await
        .expect("connect stale callback");
    stale_stream
        .write_all(
            b"GET /auth/callback?code=stale-code&state=wrong-state HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await
        .expect("write stale callback");
    let mut stale_response = String::new();
    stale_stream
        .read_to_string(&mut stale_response)
        .await
        .expect("read stale callback response");
    assert!(stale_response.contains("state mismatch"));

    let mut valid_stream = tokio::net::TcpStream::connect(("127.0.0.1", callback_port))
        .await
        .expect("connect valid callback");
    valid_stream
        .write_all(
            b"GET /auth/callback?code=valid-code&state=expected-state HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await
        .expect("write valid callback");
    let mut valid_response = String::new();
    valid_stream
        .read_to_string(&mut valid_response)
        .await
        .expect("read valid callback response");
    assert!(valid_response.contains("Authentication complete"));

    let result = listener_task.await.expect("listener task");
    assert!(matches!(
        result,
        Ok(OAuthCallbackOutcome::Authorized { code, state })
            if code == "valid-code" && state == "expected-state"
    ));
}
