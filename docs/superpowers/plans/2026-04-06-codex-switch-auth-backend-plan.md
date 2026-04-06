# Codex Switch Auth Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first real backend-powered Codex Switch macOS app with app-owned OAuth account import, local multi-account persistence, live `~/.codex/auth.json` switching, quota refresh, and a Codex-specific status bar menu.

**Architecture:** Add a dedicated Codex account subsystem in Rust and React instead of reusing the generic provider model. The backend owns OAuth, local account persistence, file watching, quota refresh, and tray updates; the frontend becomes a thin renderer over Tauri commands and events.

**Tech Stack:** Tauri 2, Rust, React, TypeScript, Vitest, existing SQLite/app storage infrastructure, existing tray integration, existing Codex auth parsing and subscription logic.

---

## File Structure

### New Rust Files

- Create: `src-tauri/src/codex_accounts/mod.rs`
  Responsibility: expose the Codex account subsystem and shared types.
- Create: `src-tauri/src/codex_accounts/model.rs`
  Responsibility: persisted account types, quota snapshot types, OAuth session types.
- Create: `src-tauri/src/codex_accounts/store.rs`
  Responsibility: load/save Codex accounts from app-local storage.
- Create: `src-tauri/src/codex_accounts/auth_file.rs`
  Responsibility: parse and write Codex-compatible `auth.json` snapshots while preserving unknown fields.
- Create: `src-tauri/src/codex_accounts/oauth.rs`
  Responsibility: PKCE generation, auth URL building, localhost callback listener, token exchange.
- Create: `src-tauri/src/codex_accounts/watcher.rs`
  Responsibility: watch `~/.codex/auth.json`, debounce changes, and sync active account state.
- Create: `src-tauri/src/commands/codex_accounts.rs`
  Responsibility: Tauri command surface for the frontend.
- Create: `src-tauri/tests/codex_accounts_commands.rs`
  Responsibility: command-level integration tests.
- Create: `src-tauri/tests/codex_accounts_oauth.rs`
  Responsibility: OAuth state validation and callback handling tests.
- Create: `src-tauri/tests/codex_accounts_store.rs`
  Responsibility: persistence and bootstrapping tests.

### Modified Rust Files

- Modify: `src-tauri/src/lib.rs`
  Responsibility: register new module, manage subsystem lifecycle, expose commands/events, wire tray refresh and startup bootstrap.
- Modify: `src-tauri/src/store.rs`
  Responsibility: add Codex account manager state handles.
- Modify: `src-tauri/src/tray.rs`
  Responsibility: replace generic Codex tray section with Codex Switch account menu for this feature.
- Modify: `src-tauri/src/services/subscription.rs`
  Responsibility: extract reusable per-account quota fetching path from current live-account-only logic.
- Modify: `src-tauri/src/commands/mod.rs`
  Responsibility: export Codex account commands.

### New Frontend Files

- Create: `src/features/codex-switch/api.ts`
  Responsibility: typed invoke wrappers and event subscriptions for Codex accounts.
- Create: `src/features/codex-switch/hooks/useCodexAccounts.ts`
  Responsibility: frontend state management for account list, add-account pending flow, switching, delete, refresh.
- Create: `src/features/codex-switch/types-runtime.ts`
  Responsibility: runtime DTOs matching Tauri command payloads.

### Modified Frontend Files

- Modify: `src/features/codex-switch/CodexSwitchApp.tsx`
  Responsibility: replace mock state with hook-driven backend state.
- Modify: `src/features/codex-switch/mockAccounts.ts`
  Responsibility: delete once runtime data is fully wired.
- Modify: `src/features/codex-switch/components/CodexAccountsView.tsx`
  Responsibility: render backend data, pending add-account state, and refresh/switch/delete actions.
- Modify: `src/features/codex-switch/components/CodexAccountCard.tsx`
  Responsibility: consume runtime account DTOs and richer quota/loading/error states.
- Modify: `tests/integration/App.test.tsx`
  Responsibility: mock Tauri account commands/events instead of mock account constants.

### Assets

- Replace: `src-tauri/icons/*`
  Responsibility: white-background rudder app icon set.
- Replace: `src-tauri/icons/tray/macos/statusTemplate.png`
- Replace: `src-tauri/icons/tray/macos/statusTemplate@2x.png`
- Replace: `src-tauri/icons/tray/macos/statusbar_template_3x.png`
  Responsibility: monochrome template-style rudder tray icons.

## Task 1: Create the Codex Account Domain and Persistence Layer

**Files:**
- Create: `src-tauri/src/codex_accounts/mod.rs`
- Create: `src-tauri/src/codex_accounts/model.rs`
- Create: `src-tauri/src/codex_accounts/store.rs`
- Test: `src-tauri/tests/codex_accounts_store.rs`

- [ ] **Step 1: Write the failing persistence tests**

Add tests covering:
- bootstrap from existing `~/.codex/auth.json`
- upsert by `account_id`
- only one `is_active` account at a time
- delete refuses active account

Example test skeleton:

```rust
#[test]
fn bootstrap_imports_live_auth_as_active_account() {
    let temp = tempfile::tempdir().unwrap();
    let live_auth_path = temp.path().join("auth.json");
    std::fs::write(&live_auth_path, SAMPLE_AUTH_JSON).unwrap();

    let mut store = CodexAccountStore::new(temp.path().join("codex-accounts.json"));
    let imported = store.bootstrap_from_live_auth(&live_auth_path).unwrap();

    assert_eq!(imported.email.as_deref(), Some("earnzh@gmail.com"));
    assert!(imported.is_active);
}
```

- [ ] **Step 2: Run the persistence tests to verify they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml codex_accounts_store
```

Expected:
- FAIL with missing `codex_accounts` module and missing store types

- [ ] **Step 3: Write the minimal account model types**

Implement:
- `CodexAccountRecord`
- `CodexQuotaSnapshot`
- `StoredCodexAccounts`

Include fields from the spec:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    pub metadata: serde_json::Map<String, serde_json::Value>,
}
```

- [ ] **Step 4: Implement file-backed store behavior**

Add minimal store methods:
- `load`
- `save`
- `list_accounts`
- `upsert_account`
- `set_active_account`
- `delete_account`
- `bootstrap_from_live_auth`

Store file path should live under the app config directory, for example:

```rust
pub fn codex_accounts_store_path() -> std::path::PathBuf {
    crate::config::get_app_config_dir().join("codex-accounts.json")
}
```

- [ ] **Step 5: Run the persistence tests to verify they pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml codex_accounts_store
```

Expected:
- PASS for all new persistence tests

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/codex_accounts/mod.rs src-tauri/src/codex_accounts/model.rs src-tauri/src/codex_accounts/store.rs src-tauri/tests/codex_accounts_store.rs
git commit -m "feat: add codex account persistence layer"
```

## Task 2: Parse and Reconstruct Codex `auth.json`

**Files:**
- Create: `src-tauri/src/codex_accounts/auth_file.rs`
- Modify: `src-tauri/src/codex_accounts/model.rs`
- Test: `src-tauri/tests/codex_accounts_store.rs`

- [ ] **Step 1: Write the failing auth parsing tests**

Add tests for:
- extracting email, plan, and account_id from observed JWT/auth payloads
- preserving unknown fields when reconstructing auth JSON
- writing a stored account back to live `auth.json`

Example test:

```rust
#[test]
fn parse_live_auth_extracts_email_plan_and_account_id() {
    let parsed = parse_codex_account_from_auth_json(serde_json::from_str(SAMPLE_AUTH_JSON).unwrap()).unwrap();

    assert_eq!(parsed.email.as_deref(), Some("earnzh@gmail.com"));
    assert_eq!(parsed.plan_type.as_deref(), Some("plus"));
    assert_eq!(parsed.account_id.as_deref(), Some("7a569f43-4a34-45c4-8190-9580bdf63fab"));
}
```

- [ ] **Step 2: Run the auth parsing tests to verify they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml parse_live_auth
```

Expected:
- FAIL with missing parsing functions

- [ ] **Step 3: Implement auth parsing and reconstruction helpers**

Add:
- `parse_codex_account_from_auth_json`
- `write_live_codex_auth_json`
- JWT payload decoding helper using base64url decoding only, no signature verification
- preservation of original auth JSON for unknown fields

The implementation should avoid normalizing away fields outside the known set:

```rust
pub fn parse_codex_account_from_auth_json(auth_json: serde_json::Value) -> Result<CodexAccountRecord, AppError> {
    // extract auth_mode, tokens.account_id, email/profile claims, plan claims
    // keep original full auth_json in the record
}
```

- [ ] **Step 4: Run the auth parsing tests to verify they pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml parse_live_auth
```

Expected:
- PASS for new parsing tests

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/codex_accounts/auth_file.rs src-tauri/src/codex_accounts/model.rs src-tauri/tests/codex_accounts_store.rs
git commit -m "feat: parse and reconstruct codex auth snapshots"
```

## Task 3: Implement App-Owned OAuth with Localhost Callback

**Files:**
- Create: `src-tauri/src/codex_accounts/oauth.rs`
- Create: `src-tauri/tests/codex_accounts_oauth.rs`

- [ ] **Step 1: Write the failing OAuth state tests**

Cover:
- PKCE generation
- auth URL includes expected parameters
- callback state mismatch fails
- callback success returns captured code

Example test:

```rust
#[tokio::test]
async fn callback_rejects_mismatched_state() {
    let session = OAuthSession::new_for_test("expected-state".into(), "verifier".into(), 1455);
    let result = session.handle_callback_query("code=abc&state=wrong").await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run the OAuth tests to verify they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml codex_accounts_oauth
```

Expected:
- FAIL with missing OAuth session types

- [ ] **Step 3: Implement minimal OAuth session support**

Add:
- PKCE generator
- auth URL builder
- localhost listener with one-shot completion
- token exchange HTTP call wrapper
- timeout and cancellation handling

Keep URL generation isolated:

```rust
pub fn build_authorize_url(session: &OAuthSession) -> Result<url::Url, AppError> {
    // use the observed auth.openai.com authorize pattern
}
```

- [ ] **Step 4: Run the OAuth tests to verify they pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml codex_accounts_oauth
```

Expected:
- PASS for callback and PKCE tests

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/codex_accounts/oauth.rs src-tauri/tests/codex_accounts_oauth.rs
git commit -m "feat: add codex oauth callback flow"
```

## Task 4: Wire Rust Commands and Runtime State

**Files:**
- Create: `src-tauri/src/commands/codex_accounts.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/store.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/codex_accounts_commands.rs`

- [ ] **Step 1: Write the failing command tests**

Cover:
- list accounts returns bootstrapped live account
- start OAuth emits pending session state
- switch account rewrites live auth file
- delete active account fails

- [ ] **Step 2: Run the command tests to verify they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml codex_accounts_commands
```

Expected:
- FAIL with unregistered commands

- [ ] **Step 3: Implement command handlers**

Add commands:
- `list_codex_accounts`
- `get_active_codex_account`
- `start_codex_account_oauth`
- `cancel_codex_account_oauth`
- `switch_codex_account`
- `delete_codex_account`

Also:
- extend `AppState` with Codex account manager state
- bootstrap from live `auth.json` during app startup
- register commands in `invoke_handler`

- [ ] **Step 4: Run the command tests to verify they pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml codex_accounts_commands
```

Expected:
- PASS for new command tests

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/codex_accounts.rs src-tauri/src/commands/mod.rs src-tauri/src/store.rs src-tauri/src/lib.rs src-tauri/tests/codex_accounts_commands.rs
git commit -m "feat: wire codex account tauri commands"
```

## Task 5: Add Real-Time `auth.json` Watching

**Files:**
- Create: `src-tauri/src/codex_accounts/watcher.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/store.rs`
- Test: `src-tauri/tests/codex_accounts_store.rs`

- [ ] **Step 1: Write the failing watcher tests**

Cover:
- file change upserts a new account
- file change marks account active
- duplicate burst events debounce correctly

- [ ] **Step 2: Run the watcher tests to verify they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml watcher
```

Expected:
- FAIL with missing watcher module

- [ ] **Step 3: Implement watcher service**

Use a lightweight polling loop if the repo does not already have a preferred watcher abstraction. Keep it simple:
- poll modtime/hash
- debounce duplicate changes
- emit `codex-accounts-updated`

- [ ] **Step 4: Run the watcher tests to verify they pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml watcher
```

Expected:
- PASS for watcher tests

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/codex_accounts/watcher.rs src-tauri/src/lib.rs src-tauri/src/store.rs src-tauri/tests/codex_accounts_store.rs
git commit -m "feat: sync codex accounts from live auth changes"
```

## Task 6: Generalize Quota Refresh to Saved Accounts

**Files:**
- Modify: `src-tauri/src/services/subscription.rs`
- Modify: `src-tauri/src/codex_accounts/model.rs`
- Modify: `src-tauri/src/commands/codex_accounts.rs`
- Test: `src-tauri/tests/codex_accounts_commands.rs`

- [ ] **Step 1: Write the failing quota tests**

Cover:
- refresh one account updates saved quota snapshot
- refresh all accounts preserves old quota on network failure
- expired token reports a distinct error

- [ ] **Step 2: Run the quota tests to verify they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml quota
```

Expected:
- FAIL because refresh commands and per-account token usage do not exist

- [ ] **Step 3: Implement per-account quota fetching**

Extract the current Codex credential reading logic into a reusable function that accepts a stored auth snapshot instead of only reading the live environment.

Add commands:
- `refresh_codex_account_quota`
- `refresh_all_codex_account_quotas`

- [ ] **Step 4: Run the quota tests to verify they pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml quota
```

Expected:
- PASS for quota tests

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/subscription.rs src-tauri/src/codex_accounts/model.rs src-tauri/src/commands/codex_accounts.rs src-tauri/tests/codex_accounts_commands.rs
git commit -m "feat: refresh quota for saved codex accounts"
```

## Task 7: Replace Frontend Mock Data with Tauri-Driven State

**Files:**
- Create: `src/features/codex-switch/api.ts`
- Create: `src/features/codex-switch/hooks/useCodexAccounts.ts`
- Create: `src/features/codex-switch/types-runtime.ts`
- Modify: `src/features/codex-switch/CodexSwitchApp.tsx`
- Modify: `src/features/codex-switch/components/CodexAccountsView.tsx`
- Modify: `src/features/codex-switch/components/CodexAccountCard.tsx`
- Modify: `tests/integration/App.test.tsx`

- [ ] **Step 1: Write the failing frontend tests**

Replace mock-account assumptions with command/event mocks.

Cover:
- initial backend-driven render
- add-account pending state
- switch action invokes Tauri command
- delete action invokes Tauri command
- refresh action invokes Tauri command

- [ ] **Step 2: Run the frontend tests to verify they fail**

Run:

```bash
corepack pnpm vitest run tests/integration/App.test.tsx
```

Expected:
- FAIL because the app still depends on mock data

- [ ] **Step 3: Implement minimal frontend runtime bindings**

Add typed invoke wrappers:

```ts
export async function listCodexAccounts(): Promise<CodexAccountDto[]> {
  return invoke("list_codex_accounts");
}
```

Add `useCodexAccounts` to:
- load accounts
- subscribe to backend events
- expose add, switch, delete, refresh actions

Remove `mockAccounts` usage from the main Codex Switch shell.

- [ ] **Step 4: Run the frontend tests to verify they pass**

Run:

```bash
corepack pnpm vitest run tests/integration/App.test.tsx
corepack pnpm typecheck
```

Expected:
- PASS for integration tests
- PASS for typecheck

- [ ] **Step 5: Commit**

```bash
git add src/features/codex-switch/api.ts src/features/codex-switch/hooks/useCodexAccounts.ts src/features/codex-switch/types-runtime.ts src/features/codex-switch/CodexSwitchApp.tsx src/features/codex-switch/components/CodexAccountsView.tsx src/features/codex-switch/components/CodexAccountCard.tsx tests/integration/App.test.tsx
git commit -m "feat: connect codex switch ui to backend accounts"
```

## Task 8: Build the macOS Status Bar Menu

**Files:**
- Modify: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/codex_accounts_commands.rs`

- [ ] **Step 1: Write the failing tray tests**

Cover:
- tray menu includes open, refresh, saved accounts, quit
- active account menu item is disabled
- inactive account menu item is actionable

- [ ] **Step 2: Run the tray tests to verify they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml tray
```

Expected:
- FAIL because the Codex tray section is still generic

- [ ] **Step 3: Implement Codex Switch tray menu generation**

Replace the current Codex-specific tray subsection with:
- `Open Codex Switch`
- `Refresh Quota`
- saved accounts by email
- `Quit`

Wire menu events to:
- show main window
- refresh quotas
- switch account
- quit app

- [ ] **Step 4: Run the tray tests to verify they pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml tray
```

Expected:
- PASS for tray tests

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/tray.rs src-tauri/src/lib.rs src-tauri/tests/codex_accounts_commands.rs
git commit -m "feat: add codex switch status bar menu"
```

## Task 9: Replace App and Tray Icons with Rudder Assets

**Files:**
- Modify: `src-tauri/icons/*`
- Modify: `src-tauri/icons/tray/macos/*`

- [ ] **Step 1: Add new rudder-style icon assets**

Create a white-background app icon set and template-style monochrome tray icons sized to match current file names.

- [ ] **Step 2: Run a local build check for icon packaging**

Run:

```bash
corepack pnpm tauri build --debug
```

Expected:
- build succeeds
- no icon-asset packaging errors

- [ ] **Step 3: Commit**

```bash
git add src-tauri/icons src-tauri/icons/tray/macos
git commit -m "feat: add rudder app and tray icons"
```

## Task 10: Final End-to-End Verification and Packaging

**Files:**
- Modify: `package.json` if a dedicated DMG script is needed
- Modify: `src-tauri/tauri.conf.json` only if packaging metadata needs updates

- [ ] **Step 1: Run focused Rust verification**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml codex_accounts
```

Expected:
- PASS for new Codex account tests

- [ ] **Step 2: Run frontend verification**

Run:

```bash
corepack pnpm vitest run tests/integration/App.test.tsx
corepack pnpm typecheck
```

Expected:
- PASS

- [ ] **Step 3: Manually verify the full flow**

Verify:
- startup imports current live account
- add-account opens browser and stores a second account without switching
- switch writes the saved auth snapshot to `~/.codex/auth.json`
- tray menu shows account emails and disables the active one
- refresh updates quota data in both tray and window

- [ ] **Step 4: Build the macOS app bundle or DMG**

Run:

```bash
corepack pnpm tauri build
```

Expected:
- macOS app build succeeds
- app bundle includes new icons and tray icon assets

- [ ] **Step 5: Commit**

```bash
git add package.json src-tauri/tauri.conf.json
git commit -m "chore: finalize codex switch auth backend release"
```

## Self-Review

### Spec Coverage

- OAuth add-account flow: covered by Tasks 3, 4, and 7
- local multi-account persistence: covered by Tasks 1 and 2
- startup bootstrap from live auth: covered by Tasks 1 and 4
- explicit switching via `~/.codex/auth.json`: covered by Tasks 2 and 4
- real-time live auth monitoring: covered by Task 5
- quota refresh: covered by Task 6
- frontend integration: covered by Task 7
- status bar menu: covered by Task 8
- rudder app/tray icons: covered by Task 9
- macOS packaging verification: covered by Task 10

### Placeholder Scan

- No `TODO`, `TBD`, or deferred “write tests later” steps remain.
- Each task includes concrete files, commands, and expected results.

### Type Consistency

- Rust domain naming stays under `CodexAccountRecord`, `CodexQuotaSnapshot`, and dedicated `codex_accounts` modules.
- Frontend runtime naming stays under `CodexAccountDto`-style command payloads and `useCodexAccounts`.
- Commands align with the spec command surface.
