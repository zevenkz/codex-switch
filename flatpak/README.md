# Flatpak

Flatpak packaging files for Codex Switch.

## Build

```bash
corepack pnpm build
cp "$(find src-tauri/target/release/bundle -name '*.deb' | head -n 1)" flatpak/codex-switch.deb
flatpak-builder --force-clean --user --disable-cache --repo flatpak-repo flatpak-build flatpak/com.codexswitch.desktop.yml
```
