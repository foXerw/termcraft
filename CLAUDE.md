# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

TermCraft is a desktop terminal/SSH/Telnet client built with **Tauri v2** (Rust backend + React frontend). The UI is localized in Chinese (zh-CN).

## Build & Run Commands

```bash
cd termcraft
npm run tauri dev       # Development mode with hot reload
npm run tauri build     # Production build + bundle
npm run dev             # Vite dev server only (port 1420), no Rust backend
npm run build           # Frontend-only build (tsc && vite build)
```

No test, lint, or format commands exist. No test infrastructure is set up.

## Architecture

### Frontend-Backend IPC

All communication uses Tauri's IPC bridge:

- **Streaming data (terminal output)**: Uses `Tauri Channel` objects. Frontend creates a `Channel`, passes it to a Rust command via `invoke()`, then binds `channel.onmessage` to `xterm.write()`. Stored in `appStore.channels`.
- **Command-style requests**: Standard `invoke()` calls for CRUD operations, preset execution, etc.

### Frontend Stack

- React 18 + Ant Design 5 + Zustand 4 (3 stores: `appStore`, `connectionStore`, `presetStore`)
- xterm.js 5 + WebGL addon for terminal rendering
- Path alias: `@/*` → `src/*` (configured in both tsconfig.json and vite.config.ts)

### Backend Stack (Rust)

- `AppState` holds `ConnectionManager` and `Mutex<PresetEngine>`, injected via Tauri `.manage()`
- `ConnectionManager` uses `HashMap<u64, ConnectionEntry>` with `Arc<Mutex<Handler>>` per connection (SSH/Telnet/LocalShell)
- SSH: `russh` client with PTY request, data forwarding via `mpsc` + Tauri Channel
- Telnet: Raw `tokio::TcpStream` with `tokio::io::split` (note: `libtelnet-rs` is a dependency but unused)
- Local shell: `portable_pty` with `spawn_blocking` for reads
- `PresetEngine`: variable substitution (`{{name}}`), single/batch/loop execution modes, cancellation via flag

### Data Persistence

Rust backend persists JSON files atomically (write `.tmp` then rename) in `dirs::data_dir()/termcraft/`:
- `connections.json`, `presets.json`, `groups.json`, `schedules.json`, `settings.json`

### Key Entry Points

| Layer | File | Role |
|-------|------|------|
| Frontend | `src/main.tsx` → `App.tsx` → `AppLayout.tsx` | UI entry |
| Backend | `src-tauri/src/main.rs` → `lib.rs::run()` | App bootstrap |
| IPC commands | `src-tauri/src/ipc_commands.rs` | All 25+ Tauri command handlers |
| Connection handlers | `src-tauri/src/connection/{ssh,telnet,local}.rs` | Protocol implementations |
| Preset engine | `src-tauri/src/preset/engine.rs` | Command execution engine |
| Data store | `src-tauri/src/config/store.rs` | JSON file persistence |

### Incomplete Features

Several components are placeholders or TODOs:
- SettingsDialog, PresetScheduler, TemplateManager, CommandItem components (empty divs)
- SSH Agent auth: returns error "not yet implemented"
- Server key verification: `check_server_key` always returns `Ok(true)`
- Preset pause/resume: not fully implemented
- PresetScheduler: only Interval mode; Cron is TODO
- WaitCondition: defined but not used in execution
- Connection edit: `handleEdit` just logs to console