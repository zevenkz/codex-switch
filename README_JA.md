# Codex Switch

Codex Switch は、ローカル環境の Codex アカウントセッションをまとめて管理するためのデスクトップアプリです。

## 言語

- [English](./README.md)
- [简体中文](./README_ZH.md)
- [日本語](./README_JA.md)

## できること

- 現在のマシンで利用できる Codex アカウントを確認する
- 認証ファイルを手で編集せずにアクティブなアカウントを切り替える
- OAuth サインインフローで新しいアカウントを追加する
- アプリ内で利用枠やセッション関連メタデータを更新する

## ドキュメント

- [ユーザーマニュアル](./docs/user-manual/README.md)
- [コントリビューションガイド](./CONTRIBUTING.md)

## 開発

```bash
corepack pnpm install
corepack pnpm test:unit
cargo test --manifest-path src-tauri/Cargo.toml
corepack pnpm dev
```

## ビルド

```bash
corepack pnpm build
```
