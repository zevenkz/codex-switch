# Codex Switch

Codex Switch 是一个桌面应用，用来统一管理本机上的 Codex 账号会话。

## 语言

- [English](./README.md)
- [简体中文](./README_ZH.md)
- [日本語](./README_JA.md)

## 功能

- 查看当前机器上可用的 Codex 账号
- 无需手动编辑认证文件即可切换当前账号
- 通过 OAuth 登录流程添加新的账号
- 在应用内刷新账号额度与会话相关元数据

## 文档

- [用户手册](./docs/user-manual/README.md)
- [贡献指南](./CONTRIBUTING.md)

## 开发

```bash
corepack pnpm install
corepack pnpm test:unit
cargo test --manifest-path src-tauri/Cargo.toml
corepack pnpm dev
```

## 构建

```bash
corepack pnpm build
```
