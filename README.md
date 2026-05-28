# Countdown Desktop

Countdown Desktop is a local desktop app for monitoring AI usage, auditing requests, and running an OpenAI/Anthropic-compatible local API gateway. It collects Claude Code and Codex session data, records gateway traffic in SQLite, and exposes request-level analytics without sending telemetry to a hosted service.

## Features

- Local SQLite database initialization, migrations, and health checks.
- Claude Code and Codex session import with token, latency, model, stream, and request status metadata.
- Combined usage dashboards for Claude Code, Codex, OpenAI-compatible gateway traffic, and Anthropic-compatible gateway traffic.
- Request audit table with provider, model, stream mode, status, token usage, TTFT, duration, timestamps, and request detail drawer.
- OpenAI-compatible endpoints:
  - `GET /v1/models`
  - `POST /v1/responses`
  - `POST /v1/chat/completions`
- Anthropic-compatible endpoint:
  - `POST /v1/messages`
- Provider profile management for upstream base URL, API format, environment variable name, enabled state, exact models, and model prefixes.
- Streaming request recording with TTFT and duration metrics.
- Tray Quick View with today usage snapshots.
- Background sync for local Claude Code and Codex data.

## Stack

- Tauri v2
- Rust
- SQLite
- React
- TypeScript
- Vite
- Bun

## Requirements

- Bun
- Rust toolchain
- Tauri system dependencies for your OS

## Install

```bash
bun install
```

## Development

Run the web UI only:

```bash
bun run dev
```

Run the full desktop app:

```bash
bun run tauri:dev
```

Build the web assets:

```bash
bun run build
```

Build the desktop package:

```bash
bun run tauri:build
```

## Verification

Type-check the frontend:

```bash
bun run check
```

Run Rust tests:

```bash
cd src-tauri
cargo test
```

## Main Pages

### Overview

Shows local database state, usage totals, Claude Code and Codex summaries, recent requests, and histogram-style usage trends. Combined totals include `claude_code`, `codex`, `openai_compat`, and `anthropic_compat` records.

### Requests

Displays request-level audit records. The detail drawer shows provider, source mode, model, request/session ids, cwd, entrypoint, status, token usage, timing, request summary JSON, response summary JSON, and error text.

### Settings

Manages provider profiles and the local compatibility API server. Profiles define upstream routing details such as provider key, display name, base URL, API format, API key environment variable, enabled state, exact models, and model prefixes.

### Quick View

Provides a compact tray window for current local usage snapshots.

## Compatibility API

The local gateway can proxy OpenAI-style and Anthropic-style requests to configured provider profiles. Gateway traffic is written to the same request audit database as local Claude Code and Codex imports.

Default listen address:

```text
127.0.0.1:8688
```

Health check:

```bash
curl http://127.0.0.1:8688/compat/health
```

Provider API keys are read from the environment variable named in each provider profile.
