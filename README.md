# Codex Switch

Codex Switch is a desktop app for managing local Codex account sessions from one place.

## Languages

- [English](./README.md)
- [简体中文](./README_ZH.md)
- [日本語](./README_JA.md)

## What It Does

- View the Codex accounts currently available on your machine
- Switch the active account without editing auth files by hand
- Start OAuth sign-in flow for adding another account
- Refresh account quota and session-related metadata inside the app

## Documentation

- [User Manual](./docs/user-manual/README.md)
- [Contributing](./CONTRIBUTING.md)

## Development

```bash
corepack pnpm install
corepack pnpm test:unit
cargo test --manifest-path src-tauri/Cargo.toml
corepack pnpm dev
```

## Build

```bash
corepack pnpm build
```
