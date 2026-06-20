# 终端日志记录 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给每个终端（tab/连接）加会话日志记录——右键开启、选保存位置（默认带时间戳文件名）、记录输出+输入、关闭终端自动停止、终端底部显示路径并可点击用默认应用打开。

**Architecture:** 日志在后端完成。`OutputTap` 升级为多订阅者（日志与预设引擎共存）。每连接一个 `TerminalLogger`，单一写入任务用 `select!` 同时消费输出 receiver + 输入 LogChunk receiver，独占 `File` 追加。输入在 `write_to_connection` 命令里捕获。关闭即停：`TerminalView` 卸载清理调 `stop_terminal_logging`，×关闭与 shell 退出两条路都覆盖。

**Tech Stack:** Rust + Tauri v2 + `tauri-plugin-opener`；React + Ant Design 5 + Zustand + `@tauri-apps/plugin-dialog`（已有）。

## Global Constraints

- 项目无测试/无 lint/无测试基建（CLAUDE.md）。用 `cargo build --lib` + `npm run build` 编译通过 + 手动验证替代 TDD。
- UI 文案中文（zh-CN）。
- 路径别名 `@/*` → `src/*`。
- 文件 IO 全在 Rust；打开日志文件用 `@tauri-apps/plugin-opener` 的 `openPath`（前端直接调，非后端 `open_path` IPC——这是对 spec 的简化，避免 Rust opener API 不确定性，同一 capability `opener:default` 即可）。
- 不持久化日志状态；不去重输入与回显；不记录 DefaultTerminal（无 tab）。
- 多订阅者 tap：`subscribe_output` 返回 `(sub_id, receiver)`，`unsubscribe_output(id, sub_id)` 按 id 移除。

参考 spec：`docs/superpowers/specs/2026-06-20-terminal-logging-design.md`

---

## File Structure

| 文件 | 责任 | 动作 |
|------|------|------|
| `src-tauri/Cargo.toml` | Rust 依赖 | 修改：加 `tauri-plugin-opener` |
| `src-tauri/src/lib.rs` | builder + invoke_handler | 修改：注册 opener 插件；加 2 个新命令注册 |
| `src-tauri/capabilities/default.json` | 权限 | 修改：加 `opener:default` |
| `src-tauri/src/connection/mod.rs` | OutputTap 类型 + tap_send | 修改：多订阅者 |
| `src-tauri/src/connection/manager.rs` | 连接管理 + 日志方法 | 修改：subscribe/unsubscribe 带 sub_id；loggers map + start/stop/log_input |
| `src-tauri/src/connection/logger.rs` | TerminalLogger 写入任务 | 新建 |
| `src-tauri/src/preset/engine.rs` | 预设执行 | 修改：适配 sub_id |
| `src-tauri/src/ipc_commands.rs` | IPC | 修改：加 2 命令；write_to_connection 加 log_input |
| `src/stores/appStore.ts` | 前端状态 | 修改：logPaths map + actions |
| `src/components/layout/TabBar.tsx` | 标签栏 | 修改：右键菜单 + 开始流程 |
| `src/components/terminal/TerminalView.tsx` | 终端视图 | 修改：底部路径条 + 点击打开 + 卸载停止 |
| `package.json` | JS 依赖 | 修改：加 `@tauri-apps/plugin-opener` |

---

### Task 1: 加 `tauri-plugin-opener` 依赖与权限

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `package.json`

**Interfaces:**
- Produces: opener 插件已注册；前端可 `import { openPath } from "@tauri-apps/plugin-opener"`。

- [ ] **Step 1: Cargo.toml 加依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 末尾（`tauri-plugin-dialog` 行之后）加：

```toml
tauri-plugin-opener = "2"
```

- [ ] **Step 2: lib.rs 注册插件**

在 `src-tauri/src/lib.rs` 的 `tauri::Builder::default()` 链中，`.plugin(tauri_plugin_dialog::init())` 之后加一行：

```rust
        .plugin(tauri_plugin_opener::init())
```

- [ ] **Step 3: capabilities 加权限**

在 `src-tauri/capabilities/default.json` 的 `permissions` 数组加 `"opener:default"`。改后大致：

```json
  "permissions": [
    "core:default",
    "shell:allow-open",
    "dialog:default",
    "opener:default",
    {
      "identifier": "shell:allow-execute",
      "allow": [
        { "cmd": "", "name": "", "sidecar": false }
      ]
    }
  ]
```

- [ ] **Step 4: 装 opener JS 插件**

Run: `npm install @tauri-apps/plugin-opener`
Expected: `package.json` 出现 `"@tauri-apps/plugin-opener": "^2"`。

- [ ] **Step 5: 编译验证**

Run: `cd src-tauri && cargo build --lib`
Expected: 编译通过（若 termcraft.exe 被占用用 `--lib`）。

Run: `npm run build`
Expected: tsc + vite 通过。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/capabilities/default.json package.json package-lock.json
git commit -m "feat(logging): add tauri-plugin-opener for opening log files"
```

---

### Task 2: OutputTap 升级为多订阅者 + 适配预设引擎

> 此任务把 tap 改成多订阅者，同时改 manager 的 subscribe/unsubscribe 签名与预设引擎调用点，**一起编译一起提交**（签名变更会让旧调用点编译失败）。

**Files:**
- Modify: `src-tauri/src/connection/mod.rs`
- Modify: `src-tauri/src/connection/manager.rs`
- Modify: `src-tauri/src/preset/engine.rs`

**Interfaces:**
- Produces:
  - `OutputTap` = `Arc<StdMutex<OutputTapInner>>`，`OutputTapInner { senders: Vec<(u64, UnboundedSender<Vec<u8>>)>, next_sub_id: u64 }`
  - `pub fn new_output_tap() -> OutputTap`（返回空 inner）
  - `pub fn tap_send(tap: &OutputTap, bytes: &[u8])`（扇出，清理失败 sender）
  - `ConnectionManager::subscribe_output(&self, id: &str) -> Option<(u64, UnboundedReceiver<Vec<u8>>)>`
  - `ConnectionManager::unsubscribe_output(&self, id: &str, sub_id: u64)`
- handler 文件（ssh/telnet/local）**不改**：它们只用 `new_output_tap()`、`tap_send(&tap, &bytes)`、`output_tap()`，签名不变。

- [ ] **Step 1: mod.rs 改 OutputTap 类型与函数**

在 `src-tauri/src/connection/mod.rs`，找到：

```rust
/// Output tap: an optional subscriber that receives a copy of every byte the
/// connection produces, in addition to the normal frontend (xterm) channel.
/// Used by the preset engine to capture per-command output for matching.
/// `None` means "no subscriber" — bytes only go to the frontend (the default).
pub type OutputTap = Arc<StdMutex<Option<UnboundedSender<Vec<u8>>>>>;

/// Create a fresh, empty tap (no subscriber).
pub fn new_output_tap() -> OutputTap {
    Arc::new(StdMutex::new(None))
}

/// Forward a copy of `bytes` to the tap's subscriber, if any. Never blocks:
/// unbounded channel; errors (subscriber dropped) are ignored.
pub fn tap_send(tap: &OutputTap, bytes: &[u8]) {
    if let Ok(guard) = tap.lock() {
        if let Some(sender) = guard.as_ref() {
            let _ = sender.send(bytes.to_vec());
        }
    }
}
```

替换为：

```rust
/// Output tap: a set of subscribers, each receiving a copy of every byte the
/// connection produces (in addition to the normal frontend/xterm channel).
/// Multi-subscriber so the preset engine and terminal logging can both listen
/// without one clobbering the other. Each subscriber is identified by a
/// monotonically-increasing `sub_id` returned from `subscribe_output` (in the
/// manager) and removed via `unsubscribe_output`.
pub struct OutputTapInner {
    pub senders: Vec<(u64, UnboundedSender<Vec<u8>>)>,
    pub next_sub_id: u64,
}

pub type OutputTap = Arc<StdMutex<OutputTapInner>>;

/// Create a fresh, empty tap (no subscribers).
pub fn new_output_tap() -> OutputTap {
    Arc::new(StdMutex::new(OutputTapInner {
        senders: Vec::new(),
        next_sub_id: 0,
    }))
}

/// Forward a copy of `bytes` to every tap subscriber. Never blocks: unbounded
/// channels; subscribers whose receiver has been dropped are pruned.
pub fn tap_send(tap: &OutputTap, bytes: &[u8]) {
    if let Ok(mut guard) = tap.lock() {
        guard.senders.retain(|(_, sender)| sender.send(bytes.to_vec()).is_ok());
    }
}
```

- [ ] **Step 2: manager.rs 改 subscribe/unsubscribe**

在 `src-tauri/src/connection/manager.rs`，找到 `subscribe_output`：

```rust
    /// Subscribe to a connection's output stream. Returns a receiver that yields
    /// a copy of every byte the connection produces. Replaces any existing
    /// subscriber. Drop the receiver or call `unsubscribe_output` when done.
    pub async fn subscribe_output(&self, id: &str) -> Option<UnboundedReceiver<Vec<u8>>> {
        let tap = self.tap_of(id).await?;
        let (tx, rx) = unbounded_channel();
        if let Ok(mut guard) = tap.lock() {
            *guard = Some(tx);
        }
        Some(rx)
    }

    /// Remove the output subscriber from a connection (e.g. when a preset run ends).
    pub async fn unsubscribe_output(&self, id: &str) {
        if let Some(tap) = self.tap_of(id).await {
            if let Ok(mut guard) = tap.lock() {
                *guard = None;
            }
        }
    }
```

替换为：

```rust
    /// Subscribe to a connection's output stream. Returns a `(sub_id, receiver)`
    /// pair; the receiver yields a copy of every byte the connection produces.
    /// Multiple subscribers may exist concurrently (preset engine + terminal
    /// logger). Remove a specific subscriber with `unsubscribe_output(id, sub_id)`.
    pub async fn subscribe_output(&self, id: &str) -> Option<(u64, UnboundedReceiver<Vec<u8>>)> {
        let tap = self.tap_of(id).await?;
        let (tx, rx) = unbounded_channel();
        let sub_id = {
            let mut guard = tap.lock().ok()?;
            let id = guard.next_sub_id;
            guard.next_sub_id += 1;
            guard.senders.push((id, tx));
            id
        };
        Some((sub_id, rx))
    }

    /// Remove a specific output subscriber (by `sub_id`) from a connection.
    pub async fn unsubscribe_output(&self, id: &str, sub_id: u64) {
        if let Some(tap) = self.tap_of(id).await {
            if let Ok(mut guard) = tap.lock() {
                guard.senders.retain(|(sid, _)| *sid != sub_id);
            }
        }
    }
```

- [ ] **Step 3: engine.rs 适配 sub_id**

在 `src-tauri/src/preset/engine.rs`，找到（约 84 行）：

```rust
        let rx = match manager.subscribe_output(&connection_id).await {
            Some(r) => r,
            None => {
```

改为：

```rust
        let (sub_id, rx) = match manager.subscribe_output(&connection_id).await {
            Some(pair) => pair,
            None => {
```

并在该函数末尾（约 103 行）：

```rust
        // Always release the output subscriber.
        manager.unsubscribe_output(&connection_id).await;
```

改为：

```rust
        // Always release the output subscriber.
        manager.unsubscribe_output(&connection_id, sub_id).await;
```

- [ ] **Step 4: 编译验证**

Run: `cd src-tauri && cargo build --lib`
Expected: 编译通过，无 `subscribe_output` / `unsubscribe_output` 签名不匹配错误。确认无其他文件残留旧签名调用（grep `unsubscribe_output\|subscribe_output` 应只有 manager.rs、engine.rs 两处）。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/connection/mod.rs src-tauri/src/connection/manager.rs src-tauri/src/preset/engine.rs
git commit -m "refactor(connection): make OutputTap multi-subscriber

subscribe_output now returns (sub_id, receiver) and appends instead of
replacing, so the preset engine and terminal logger can listen to the
same connection simultaneously. unsubscribe_output takes the sub_id."
```

---

### Task 3: TerminalLogger 模块 + ConnectionManager 日志方法

**Files:**
- Create: `src-tauri/src/connection/logger.rs`
- Modify: `src-tauri/src/connection/mod.rs`（加 `pub mod logger;`）
- Modify: `src-tauri/src/connection/manager.rs`（loggers map + 方法）

**Interfaces:**
- Consumes: Task 2 的 `subscribe_output -> (u64, rx)`、`unsubscribe_output(id, sub_id)`。
- Produces:
  - `pub enum LogChunk { Output(Vec<u8>), Input(Vec<u8>) }`
  - `pub struct LoggerHandle { sender: UnboundedSender<LogChunk>, out_sub_id: u64 }`
  - `LoggerHandle::start(path: &str, rx_out: UnboundedReceiver<Vec<u8>>, out_sub_id: u64) -> Result<Self, AppError>`
  - `ConnectionManager::start_logging(&self, id: &str, path: String) -> Result<(), AppError>`
  - `ConnectionManager::stop_logging(&self, id: &str)`（幂等）
  - `ConnectionManager::log_input(&self, id: &str, data: &[u8])`

- [ ] **Step 1: 新建 logger.rs**

创建 `src-tauri/src/connection/logger.rs`：

```rust
use std::fs::OpenOptions;
use std::io::Write;

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::errors::AppError;

/// A chunk of session data to be appended to the log file.
#[derive(Debug)]
pub enum LogChunk {
    Output(Vec<u8>),
    Input(Vec<u8>),
}

/// Handle for an active per-connection log. Dropping the `sender` (e.g. by
/// removing the handle from the manager) ends the writer task and closes the
/// file. `out_sub_id` is used to detach the output-tap subscriber on stop.
pub struct LoggerHandle {
    pub sender: UnboundedSender<LogChunk>,
    pub out_sub_id: u64,
}

impl LoggerHandle {
    /// Open `path` for append and spawn a writer task that consumes both the
    /// connection's output stream (`rx_out`) and input chunks (`log_rx`,
    /// fed by `log_input`), writing them to the file in arrival order.
    pub fn start(
        path: &str,
        mut rx_out: UnboundedReceiver<Vec<u8>>,
        out_sub_id: u64,
    ) -> Result<Self, AppError> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .truncate(false)
            .open(path)
            .map_err(|e| AppError::Connection(format!("打开日志文件失败: {}", e)))?;

        let (log_tx, mut log_rx) = unbounded_channel::<LogChunk>();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    out = rx_out.recv() => match out {
                        Some(bytes) => {
                            let _ = file.write_all(&bytes);
                            let _ = file.flush();
                        }
                        None => break,
                    },
                    inp = log_rx.recv() => match inp {
                        Some(LogChunk::Output(bytes)) | Some(LogChunk::Input(bytes)) => {
                            let _ = file.write_all(&bytes);
                            let _ = file.flush();
                        }
                        None => break,
                    },
                }
            }
        });

        Ok(LoggerHandle {
            sender: log_tx,
            out_sub_id,
        })
    }
}
```

- [ ] **Step 2: mod.rs 注册 logger 模块**

在 `src-tauri/src/connection/mod.rs` 顶部 `pub mod` 区（`pub mod manager;` 等）加一行：

```rust
pub mod logger;
```

- [ ] **Step 3: manager.rs 加 loggers map 与方法**

在 `src-tauri/src/connection/manager.rs` 顶部 import 区加：

```rust
use crate::connection::logger::{LogChunk, LoggerHandle};
```

找到 `ConnectionManager` 结构与构造：

```rust
pub struct ConnectionManager {
    connections: Mutex<HashMap<String, ConnectionEntry>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }
```

改为：

```rust
pub struct ConnectionManager {
    connections: Mutex<HashMap<String, ConnectionEntry>>,
    loggers: Mutex<HashMap<String, LoggerHandle>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            loggers: Mutex::new(HashMap::new()),
        }
    }
```

在 `impl ConnectionManager { ... }` 末尾（`resize` 方法之后）加三个方法：

```rust
    /// Start logging a connection's output+input to `path`. Fails if the
    /// connection doesn't exist or is already being logged.
    pub async fn start_logging(&self, id: &str, path: String) -> Result<(), AppError> {
        {
            let loggers = self.loggers.lock().await;
            if loggers.contains_key(id) {
                return Err(AppError::Connection("该终端已在记录日志".to_string()));
            }
        }
        let (sub_id, rx_out) = match self.subscribe_output(id).await {
            Some(pair) => pair,
            None => {
                return Err(AppError::NotFound(format!(
                    "Connection {} not found",
                    id
                )))
            }
        };
        let handle = LoggerHandle::start(&path, rx_out, sub_id)?;
        self.loggers.lock().await.insert(id.to_string(), handle);
        Ok(())
    }

    /// Stop logging a connection (idempotent). Drops the writer sender (ends the
    /// writer task, closing the file) and detaches the output-tap subscriber.
    pub async fn stop_logging(&self, id: &str) {
        let handle = self.loggers.lock().await.remove(id);
        if let Some(handle) = handle {
            self.unsubscribe_output(id, handle.out_sub_id).await;
        }
    }

    /// Forward user input bytes to a connection's logger, if one is active.
    /// No-op (not an error) when the connection isn't being logged.
    pub async fn log_input(&self, id: &str, data: &[u8]) {
        let loggers = self.loggers.lock().await;
        if let Some(handle) = loggers.get(id) {
            let _ = handle.sender.send(LogChunk::Input(data.to_vec()));
        }
    }
```

- [ ] **Step 4: 编译验证**

Run: `cd src-tauri && cargo build --lib`
Expected: 编译通过（新方法暂未被调用，可能有 dead-code 警告，可忽略；Task 4 会用上）。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/connection/logger.rs src-tauri/src/connection/mod.rs src-tauri/src/connection/manager.rs
git commit -m "feat(logging): add TerminalLogger and ConnectionManager logging methods"
```

---

### Task 4: IPC 命令 + write_to_connection 记录输入

**Files:**
- Modify: `src-tauri/src/ipc_commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 3 的 `start_logging`/`stop_logging`/`log_input`。
- Produces:
  - `pub async fn start_terminal_logging(id: String, path: String, state) -> Result<(), String>`
  - `pub async fn stop_terminal_logging(id: String, state) -> Result<(), String>`
  - `write_to_connection` 现在也调 `log_input`。

- [ ] **Step 1: ipc_commands.rs 加两个命令**

在 `src-tauri/src/ipc_commands.rs` 找到 `write_to_connection`：

```rust
#[tauri::command]
pub async fn write_to_connection(
    id: String,
    data: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.connection_manager.write_to(&id, &data)
        .await
        .map_err(|e| e.to_string())
}
```

替换为（写入成功后转发输入到 logger）：

```rust
#[tauri::command]
pub async fn write_to_connection(
    id: String,
    data: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.connection_manager.write_to(&id, &data)
        .await
        .map_err(|e| e.to_string())?;
    // Forward the user's input to the active logger (no-op if not logging).
    state.connection_manager.log_input(&id, data.as_bytes()).await;
    Ok(())
}
```

在合适位置（如 `resize_connection` 之后）加两个新命令：

```rust
#[tauri::command]
pub async fn start_terminal_logging(
    id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .connection_manager
        .start_logging(&id, path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_terminal_logging(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.connection_manager.stop_logging(&id).await;
    Ok(())
}
```

- [ ] **Step 2: lib.rs 注册命令**

在 `src-tauri/src/lib.rs` 的 `invoke_handler!` 里，preset/连接命令区附近加：

```rust
            // Terminal logging
            ipc_commands::start_terminal_logging,
            ipc_commands::stop_terminal_logging,
```

- [ ] **Step 3: 编译验证**

Run: `cd src-tauri && cargo build --lib`
Expected: 编译通过，无 dead-code 警告（命令已注册）。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/ipc_commands.rs src-tauri/src/lib.rs
git commit -m "feat(logging): add start/stop_terminal_logging IPC; log input in write_to_connection"
```

---

### Task 5: 前端 appStore 加 logPaths

**Files:**
- Modify: `src/stores/appStore.ts`

**Interfaces:**
- Produces: `logPaths: Map<string, string>`（connectionId → 日志路径）；actions `setLogPath(connId, path)`、`clearLogPath(connId)`。

- [ ] **Step 1: 加状态与 actions**

在 `src/stores/appStore.ts` 的 `AppState` interface，`channels` 字段附近加：

```ts
  // Active log file path per connection (connectionId -> path). Drives the
  // tab right-click menu state and the terminal bottom status bar.
  logPaths: Map<string, string>;
```

在 actions 区（`setChannel`/`removeChannel` 附近）加声明：

```ts
  setLogPath: (connId: string, path: string) => void;
  clearLogPath: (connId: string) => void;
```

在 `create<AppState>((set) => ({ ... }))` 初始状态加：

```ts
  logPaths: new Map(),
```

在 actions 实现区加：

```ts
  setLogPath: (connId, path) =>
    set((state) => {
      const next = new Map(state.logPaths);
      next.set(connId, path);
      return { logPaths: next };
    }),

  clearLogPath: (connId) =>
    set((state) => {
      const next = new Map(state.logPaths);
      next.delete(connId);
      return { logPaths: next };
    }),
```

- [ ] **Step 2: 类型检查**

Run: `npm run build`
Expected: tsc 通过（本文件无新错误）。

- [ ] **Step 3: Commit**

```bash
git add src/stores/appStore.ts
git commit -m "feat(logging): add logPaths map to appStore"
```

---

### Task 6: TabBar 右键菜单 + 开始/停止流程

**Files:**
- Modify: `src/components/layout/TabBar.tsx`

**Interfaces:**
- Consumes: Task 4 的 `start_terminal_logging`/`stop_terminal_logging` IPC；`@tauri-apps/plugin-dialog` `save`；Task 5 的 `logPaths`/`setLogPath`/`clearLogPath`。
- Produces: 每个 tab 右键弹出「开始记录日志」/「停止记录日志」。

- [ ] **Step 1: 重写 TabBar（加右键菜单）**

把 `src/components/layout/TabBar.tsx` 整体替换为：

```tsx
import React from "react";
import { Dropdown, message } from "antd";
import { save } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../stores/appStore";

/** Build the default log filename: termcraft-<tabTitle>-<timestamp>.log
 *  with illegal filename chars stripped from the title. */
function defaultLogName(tabTitle: string): string {
  const safe = (tabTitle || "").replace(/[<>:"/\\|?*\x00-\x1f]/g, "").trim() || "terminal";
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const ts = `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}`;
  return `termcraft-${safe}-${ts}.log`;
}

const TabBar: React.FC = () => {
  const tabs = useAppStore((s) => s.tabs);
  const activeTabId = useAppStore((s) => s.activeTabId);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const removeTab = useAppStore((s) => s.removeTab);
  const logPaths = useAppStore((s) => s.logPaths);
  const setLogPath = useAppStore((s) => s.setLogPath);
  const clearLogPath = useAppStore((s) => s.clearLogPath);

  const startLogging = async (tab: { id: string; connectionId: string; title: string }) => {
    const path = await save({
      defaultPath: defaultLogName(tab.title),
      filters: [{ name: "日志文件", extensions: ["log", "txt"] }],
    });
    if (!path) return;
    try {
      await invoke("start_terminal_logging", { id: tab.connectionId, path });
      setLogPath(tab.connectionId, path);
      message.success("开始记录日志");
    } catch (e) {
      message.error(`开始记录失败: ${e}`);
    }
  };

  const stopLogging = async (connectionId: string) => {
    try {
      await invoke("stop_terminal_logging", { id: connectionId });
      clearLogPath(connectionId);
    } catch (e) {
      message.error(`停止记录失败: ${e}`);
    }
  };

  if (tabs.length === 0) {
    return <div className="tab-bar" style={{ flex: 1 }} />;
  }

  return (
    <div className="tab-bar" style={{ flex: 1 }}>
      {tabs.map((tab) => {
        const logging = logPaths.has(tab.connectionId);
        const items = [
          logging
            ? {
                key: "stop",
                label: "停止记录日志",
                onClick: () => stopLogging(tab.connectionId),
              }
            : {
                key: "start",
                label: "开始记录日志",
                onClick: () =>
                  startLogging({ id: tab.id, connectionId: tab.connectionId, title: tab.title }),
              },
        ];
        return (
          <Dropdown key={tab.id} trigger={["contextMenu"]} menu={{ items }}>
            <div
              className={`tab-item ${tab.id === activeTabId ? "active" : ""}`}
              onClick={() => setActiveTab(tab.id)}
              onContextMenu={(e) => e.preventDefault()}
            >
              <span style={{ color: tab.alive ? "var(--success-color)" : "var(--error-color)", fontSize: 10 }}>
                ●
              </span>
              <span>{tab.title}</span>
              <span className="close-btn" onClick={(e) => { e.stopPropagation(); removeTab(tab.id); }}>
                ×
              </span>
            </div>
          </Dropdown>
        );
      })}
    </div>
  );
};

export default TabBar;
```

- [ ] **Step 2: 类型检查**

Run: `npm run build`
Expected: tsc 通过。检查 `mcp__ide__getDiagnostics` on TabBar.tsx 为空。

- [ ] **Step 3: Commit**

```bash
git add src/components/layout/TabBar.tsx
git commit -m "feat(logging): add tab right-click menu to start/stop logging"
```

---

### Task 7: TerminalView 底部路径条 + 点击打开 + 卸载停止

**Files:**
- Modify: `src/components/terminal/TerminalView.tsx`

**Interfaces:**
- Consumes: Task 4 的 `stop_terminal_logging` IPC；`@tauri-apps/plugin-opener` `openPath`；Task 5 的 `logPaths`/`clearLogPath`。
- Produces: 当某连接在记录时，终端区下方显示日志路径条，点击用默认应用打开；卸载时停止记录。

- [ ] **Step 1: 加 import 与路径状态**

在 `src/components/terminal/TerminalView.tsx` 顶部加 import：

```tsx
import { openPath } from "@tauri-apps/plugin-opener";
import { message } from "antd";
```

（检查文件已有 antd import；若已解构 `message` 则不重复。）

在组件内（其它 `useAppStore` 选择器附近）加：

```tsx
  const logPath = useAppStore((s) => (s.logPaths.get(connectionId) ?? null));
  const clearLogPath = useAppStore((s) => s.clearLogPath);
```

- [ ] **Step 2: 卸载时停止记录**

在现有的初始化 `useEffect`（创建 Terminal 的那个）的 cleanup return 里，或新增一个 `useEffect`，加入卸载停止逻辑。新增独立 `useEffect`（依赖 `connectionId`）：

```tsx
  // Stop logging when this terminal view unmounts (tab closed via × or
  // connection_closed). Idempotent on the backend.
  useEffect(() => {
    return () => {
      import("@tauri-apps/api/core").then(({ invoke }) => {
        invoke("stop_terminal_logging", { id: connectionId }).catch(() => {});
      });
      clearLogPath(connectionId);
    };
  }, [connectionId, clearLogPath]);
```

- [ ] **Step 3: 底部路径条 + 点击打开 + fit**

把 `return` 的 JSX（当前是单个 `<div className="terminal-wrapper" ...>`）改为外层 flex 列容器 + xterm 区 + 可选日志条：

```tsx
  const openLog = async () => {
    if (!logPath) return;
    try {
      await openPath(logPath);
    } catch (e) {
      message.error(`打开日志失败: ${e}`);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", width: "100%", height: isActive ? "100%" : 0, overflow: "hidden" }}>
      <div
        className="terminal-wrapper"
        ref={terminalRef}
        style={{ flex: 1, minHeight: 0, padding: 4, display: isActive ? "block" : "none" }}
      />
      {logPath && (
        <div
          onClick={openLog}
          title="点击用默认应用打开日志文件"
          style={{
            padding: "2px 8px",
            fontSize: 11,
            color: "var(--text-secondary)",
            background: "var(--bg-secondary)",
            borderTop: "1px solid var(--border-color)",
            cursor: "pointer",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
            flexShrink: 0,
            textAlign: "left",
          }}
        >
          📄 {logPath}
        </div>
      )}
    </div>
  );
```

并加一个 `useEffect`，在 `logPath` 出现/消失时重新 fit（高度变了）：

```tsx
  // Re-fit when the log status bar appears/disappears (terminal height changes).
  useEffect(() => {
    if (isActive && fitAddonRef.current) {
      const t = setTimeout(() => fitAddonRef.current?.fit(), 50);
      return () => clearTimeout(t);
    }
  }, [logPath, isActive]);
```

- [ ] **Step 4: 类型检查**

Run: `npm run build`
Expected: tsc 通过。检查 `mcp__ide__getDiagnostics` on TerminalView.tsx 为空。

- [ ] **Step 5: Commit**

```bash
git add src/components/terminal/TerminalView.tsx
git commit -m "feat(logging): show log path bar in terminal; click to open; stop on unmount"
```

---

### Task 8: 集成手动验证

**Files:** 无（运行验证）

- [ ] **Step 1: 启动 dev**

Run: `npm run tauri dev`
Expected: 应用启动，无编译/运行错误。

- [ ] **Step 2: 右键菜单**

右键某 tab → 出现「开始记录日志」。

- [ ] **Step 3: 开始记录**

点「开始记录日志」→ save 对话框默认名形如 `termcraft-<tabTitle>-20260620-143025.log`，时间戳为当前本地时间。选位置保存 → 提示「开始记录日志」，终端底部出现路径条。

- [ ] **Step 4: 输出+输入写入**

在终端产生输出（如 `ls`、`echo hi`）+ 输入命令 → 打开日志文件确认含输出与输入字节（输入可能含回显重复，接受）。

- [ ] **Step 5: 与预设共存**

在该终端运行一个预设 → 日志照常记录输出，预设执行/匹配不受影响（多订阅者共存）。

- [ ] **Step 6: ×关闭停止**

×关闭该 tab → 日志文件停止增长；该 tab 的路径条消失。

- [ ] **Step 7: shell 退出停止**

新开一个 tab 开始记录 → 在终端输入 `exit`（触发 `connection_closed`）→ 日志停止、文件已关闭。

- [ ] **Step 8: 手动停止**

开始记录后右键 → 「停止记录日志」→ 菜单切回「开始」，底部条消失，xterm 重新 fit 不变形。

- [ ] **Step 9: 点击打开**

开始记录后点击底部路径条 → 默认应用打开日志文件。

- [ ] **Step 10: 重复开始**

对正在记录的连接再次右键开始 → 提示「该终端已在记录日志」。

- [ ] **Step 11: 不可写路径**

开始记录时选一个不可写路径（如不存在的盘符）→ `message.error`，不进入记录态。

- [ ] **Step 12: 并行记录**

两个 tab 同时开始记录 → 各自独立文件、互不干扰。

---

## Self-Review 记录

- **Spec 覆盖**：每节有对应 Task——架构(Task 2/3/4)、多订阅者 tap(Task 2)、TerminalLogger(Task 3)、IPC(Task 4)、前端状态(Task 5)、右键菜单(Task 6)、底部条+点击打开+卸载停止(Task 7)、错误边界(Task 8 验证)、文件清单(各 Task)。
- **spec 偏离（已说明）**：`open_path` IPC 改为前端直接用 `@tauri-apps/plugin-opener` 的 `openPath`（更简单，同一 `opener:default` 权限），在 Global Constraints 与 Task 7 注明。
- **占位符**：无 TBD/TODO；每步含实际代码或确切命令。
- **类型一致性**：`subscribe_output -> (u64, rx)` 在 Task 2 定义，Task 3 `start_logging` 用之；`LoggerHandle { sender, out_sub_id }` 在 Task 3 定义；`start_terminal_logging(id, path, state)` / `stop_terminal_logging(id, state)` 与前端 `{ id, path }` / `{ id }` 调用一致（Tauri 自动 camelCase）；`logPaths: Map<string,string>` + `setLogPath`/`clearLogPath` 在 Task 5 定义，Task 6/7 消费。
- **与 CLAUDE.md 一致**：无测试基建，用编译+手动验证替代 TDD。
