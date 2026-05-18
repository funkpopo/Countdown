# Countdown Desktop

Desktop application scaffold for local Claude Code and Codex analytics.

## Stack

- `Tauri v2`
- `Rust`
- `Bun`
- `Vite`
- `React`
- `TypeScript`
- `SQLite`

## Commands

```bash
bun install
bun run dev
bun run tauri:dev
bun run build
bun run tauri:build
```

## Current Scope

- Phase 0 research documents are stored in `docs/research/`
- Phase 1 scaffold provides:
  - Desktop shell bootstrap
  - Rust IPC commands
  - Local SQLite initialization
  - Planned module folders for collectors, analytics, tray, compat API, and models

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
