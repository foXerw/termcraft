# 终端日志记录设计

日期：2026-06-20
状态：已确认，待实现

## 背景

TermCraft 的终端输出流经后端 `OutputTap`（`src-tauri/src/connection/mod.rs`）转发到前端 xterm。当前 `OutputTap` 是**单订阅者**（`Option<UnboundedSender>`），`subscribe_output` 会替换现有订阅者；预设引擎（`preset/engine.rs`）在执行预设时占用它。

用户希望对每个终端记录会话日志：终端标签右键开启记录，选保存位置（默认带时间戳的文件名），记录输出+输入；关闭终端自动停止；终端窗口底部显示日志文件路径并可点击打开。

## 目标

- 每个终端（tab/连接）可独立开启/停止日志记录。
- 记录输出流 + 用户输入，追加到同一文件，按到达顺序。
- 关闭终端（×关闭标签 或 shell 退出 `connection_closed`）自动停止记录并关闭文件。
- 终端底部状态栏显示日志路径，点击用默认应用打开。
- 与预设引擎的 tap 订阅共存（互不覆盖）。

## 非目标

- 不持久化日志记录状态（重启后状态清空，磁盘文件仍在）。
- 不记录 DefaultTerminal（无 tab 的 fallback 本地终端）——入口是 tab 右键，fallback 无 tab。
- 不做日志轮转/大小限制/格式化（纯原始字节追加）。
- 不修改现有「关 tab 不 disconnect 后端」的行为（仅停止日志，不触碰连接生命周期）。
- 不去重输入与回显（多数 shell 回显输入，会产生重复字节——忠实记录，接受）。

## 架构与数据流

日志在后端完成，文件 IO 全在 Rust。每个有日志的连接持有一个 `TerminalLogger`，单一写入任务独占 `File`；输出与输入两路字节都汇入同一写入任务，按到达顺序追加。

**输出捕获**：`OutputTap` 升级为多订阅者（见下）。日志记录安装自己的输出订阅者，与预设引擎并存。

**输入捕获**：`write_to_connection` 命令里，写完连接后，若该连接有活跃 logger，把输入字节也发给其写入任务。

**TerminalLogger（每连接一个）**：
- 持有 `UnboundedSender<LogChunk>` 到一个 `tokio::spawn` 的写入任务，写入任务独占 `File`（`OpenOptions::create(true).append(true)`），循环 `rx.recv()` → `file.write_all` + `flush`。sender drop（stop）后 receiver 返回 None → 退出 → drop file（关闭）。
- 输出 tap 订阅者把 `Vec<u8>` 包成 `LogChunk::Output` 发给 sender；`write_to_connection` 把输入包成 `LogChunk::Input` 发给 sender。

**关闭即停止**：`TerminalView` 卸载清理里调 `stop_terminal_logging(id)`。×关闭标签和 `connection_closed`（shell 退出）两种路径都让 `TerminalView` 卸载 → 清理触发 → 停止日志、关闭文件。不依赖、也不改动现有 disconnect 逻辑。`stop` 幂等（已停则无操作）。

## 后端改动

### `OutputTap` 多订阅者（`src-tauri/src/connection/mod.rs`）

- 类型从 `Arc<StdMutex<Option<UnboundedSender<Vec<u8>>>>>` 改为 `Arc<StdMutex<OutputTapInner>>`，其中：
  ```rust
  struct OutputTapInner {
      senders: Vec<(u64, UnboundedSender<Vec<u8>>)>,
      next_sub_id: u64,
  }
  ```
- `new_output_tap()`：返回空 `OutputTapInner`（`next_sub_id: 0`）。
- `tap_send`：锁住，遍历，`send` 成功的保留，`Err`（接收端 dropped）的 retain 掉。
- `subscribe_output`：取 `next_sub_id`、自增、`push((sub_id, tx))`，返回 `(sub_id, receiver)`。**不再替换**。
- `unsubscribe_output(sub_id: u64)`：`retain` 掉匹配 `sub_id` 的项。

### `ConnectionManager`（`src-tauri/src/connection/manager.rs`）

- `subscribe_output` / `unsubscribe_output` 签名按上改（返回/接收 `sub_id: u64`）。
- 新增 `loggers: Mutex<HashMap<String, LoggerHandle>>`。
- 新增方法（供 IPC 调用）：
  - `start_logging(&self, id: &str, path: String) -> Result<(), AppError>`：若该 id 已有 logger → 返回错误「该终端已在记录日志」；否则建 `TerminalLogger`（开文件、spawn 写入任务、`subscribe_output` 拿 `(sub_id, rx)` 把输出转成 `LogChunk::Output` 发给写入任务），存入 `loggers` map。
  - `stop_logging(&self, id: &str)`：从 map 移除 logger（drop sender → 写入任务结束、文件关闭）；`unsubscribe_output(sub_id)` 移除输出订阅者。幂等。
  - `log_input(&self, id: &str, data: &[u8])`：若 map 有该连接 logger，发 `LogChunk::Input`；无则静默。

### `TerminalLogger` / `LoggerHandle`（新模块 `src-tauri/src/connection/logger.rs`）

```rust
pub enum LogChunk { Output(Vec<u8>), Input(Vec<u8>) }

pub struct LoggerHandle {
    sender: UnboundedSender<LogChunk>,
    out_sub_id: u64, // 用于 stop 时 unsubscribe_output
}

impl LoggerHandle {
    /// 开文件、spawn 写入任务，返回 handle。
    /// `rx_out` 是该连接输出 tap 的订阅者 receiver；写入任务用 select! 同时
    /// 消费 `rx_out`（输出，Vec<u8>）与 `log_rx`（输入，LogChunk），统一写文件。
    pub fn start(path: &str, rx_out: UnboundedReceiver<Vec<u8>>, out_sub_id: u64) -> Result<Self, AppError>;
}
```
- 写入任务：`tokio::select!` 在 `rx_out.recv()`（输出）与 `log_rx.recv()`（输入 `LogChunk::Input`）间轮询，谁先来 `file.write_all + flush`。单一消费点保证到达顺序。
- `OpenOptions::new().create(true).append(true).truncate(false).open(path)`；写失败记 log 但不中断（best-effort）。
- `stop`：drop `LoggerHandle`（其 `sender` drop → `log_rx` 结束）**且** `unsubscribe_output(out_sub_id)`（drop 输出 sender → `rx_out` 结束）→ 两条 receiver 都结束 → 写入任务退出 → drop file。

### IPC 命令（`src-tauri/src/ipc_commands.rs`）

- `start_terminal_logging(id: String, path: String) -> Result<(), String>`：调 `manager.start_logging`。
- `stop_terminal_logging(id: String) -> Result<(), String>`：调 `manager.stop_logging`。幂等。
- `open_path(path: String) -> Result<(), String>`：用 `tauri-plugin-opener` 的 `opener::open_path(path, None)` 打开（默认应用）。返回错误若 opener 失败。
- `write_to_connection`：现有写连接成功后，`manager.log_input(id, data.as_bytes())`（忽略错误/无 logger 时静默）。

### 预设引擎适配（`src-tauri/src/preset/engine.rs`）

- `subscribe_output` 现在返回 `(sub_id, rx)`：解构保存 `sub_id`，结束时 `unsubscribe_output(id, sub_id)` 替代原 `unsubscribe_output(id)`。

### 依赖

- 新增 `tauri-plugin-opener`（Rust + JS）。不用 `tauri-plugin-shell` 的 `open`——它有 scope 校验，默认拒绝任意文件路径，且日志目录由用户任意选择无法安全收窄 scope。
- `tauri.conf.json`/capabilities 配 `opener:default` 权限。

## 前端

### 状态（`src/stores/appStore.ts`）

- 新增 `logPaths: Map<string, string>`（connectionId → 当前日志文件路径），驱动右键菜单状态与底部状态栏。
- actions：`setLogPath(connId, path)`、`clearLogPath(connId)`。

### TabBar 右键菜单（`src/components/layout/TabBar.tsx`）

- 每个 tab 加 `onContextMenu`，AntD `Dropdown` + `Menu`，`trigger={['contextMenu']}`。
- 菜单项（依据 `logPaths.has(tab.connectionId)`）：
  - 未记录：`开始记录日志` → 触发开始流程。
  - 记录中：`停止记录日志` → `invoke("stop_terminal_logging", { id: tab.connectionId })` → `clearLogPath`。
- 右键哪个 tab 就操作哪个 `tab.connectionId`。

### 开始流程

1. 弹原生 `save` 对话框（`@tauri-apps/plugin-dialog`，preset 功能已加）。
2. `defaultPath` = 默认文件名：`termcraft-<tabTitle>-<时间戳>.log`
   - 时间戳本地时间 `YYYYMMDD-HHmmss`。
   - `tabTitle` 去掉非法文件名字符（`<>:"/\|?*` 与控制字符），为空则用 `terminal`。
3. 用户可改文件名/选位置，确认 → 拿到 `path`。取消则中止。
4. `invoke("start_terminal_logging", { id: connectionId, path })`。
5. 成功 → `setLogPath(connectionId, path)`（菜单切「停止」、底部栏显示）。失败 → `message.error`。

### 底部状态栏（`src/components/terminal/TerminalView.tsx`）

- 容器改 flex 列：xterm 区（`flex:1`）+ 底部日志条（`auto`，仅当 `logPaths.has(connectionId)` 时渲染）。
- 日志条靠左显示完整文件路径，小字、次要色、`cursor:pointer`。
- 点击路径 → `invoke("open_path", { path })`；失败 `message.error`。
- 路径条出现/消失后调 `fitAddon.fit()` 重算（高度变了）。
- 卸载清理（`useEffect` return）：`invoke("stop_terminal_logging", { id: connectionId })` + `clearLogPath`（幂等）。

## 错误处理与边界

- **重复开始**：某连接已有 logger → `start_logging` 返回「该终端已在记录日志」，前端 `message.warning`。
- **连接不存在**：`start_logging` 返回错误。
- **建文件失败**（路径不可写/磁盘满）：`start_logging` 返回中文错误，前端 `message.error`，不进入记录态。
- **写入失败**：写入任务 best-effort，记 log 不中断；不向用户逐条报错。
- **关闭未记录的终端**：`stop` 幂等，无操作。
- **opener 失败**：`open_path` 返回错误，前端 `message.error`。
- **时间戳**：本地时间。
- **多 tab 并行记录**：每连接独立 logger map 支持，互不干扰。
- **与预设执行共存**：多订阅者 tap 保证两者并存；预设运行期间日志照常记录输出。

## 涉及文件

| 文件 | 改动 |
|------|------|
| `src-tauri/Cargo.toml` | 加 `tauri-plugin-opener` |
| `src-tauri/src/lib.rs` | 注册 opener 插件；invoke_handler 加 3 个新命令 |
| `src-tauri/capabilities/default.json` | 加 `opener:default` |
| `src-tauri/src/connection/mod.rs` | `OutputTap` 多订阅者；`tap_send`/`subscribe_output`/`unsubscribe_output` 改造 |
| `src-tauri/src/connection/manager.rs` | `loggers` map + `start_logging`/`stop_logging`/`log_input`；subscribe 签名带 sub_id |
| `src-tauri/src/connection/logger.rs` | 新建：`LogChunk`/`LoggerHandle` + 写入任务 |
| `src-tauri/src/preset/engine.rs` | 适配 sub_id subscribe/unsubscribe |
| `src-tauri/src/ipc_commands.rs` | `start_terminal_logging`/`stop_terminal_logging`/`open_path`；`write_to_connection` 加 `log_input` |
| `src/stores/appStore.ts` | `logPaths` map + set/clear actions |
| `src/components/layout/TabBar.tsx` | 右键菜单（开始/停止） |
| `src/components/terminal/TerminalView.tsx` | 底部日志路径条 + 卸载停止 + fit |
| `package.json` | 加 `@tauri-apps/plugin-opener` |

## 测试要点

手动验证（项目无测试基建）：
1. 右键 tab → 出现「开始记录日志」。
2. 开始 → save 对话框默认名带 `termcraft-<tabTitle>-<时间戳>.log`，时间戳正确。
3. 确认 → 底部出现路径条，文件创建，内容含输出。
4. 输入命令 → 文件中含输入字节（可能含回显重复，接受）。
5. 运行预设 → 日照常记录，预设执行不受影响（多订阅者共存）。
6. ×关闭 tab → 文件停止增长、已关闭（再写无效）。
7. shell 输入 `exit` 触发 `connection_closed` → 同样停止。
8. 手动右键「停止」→ 停止，菜单切回「开始」；底部条消失；xterm 重新 fit。
9. 点击底部路径 → 默认应用打开日志文件。
10. 对同一连接重复开始 → 提示「已在记录」。
11. 建文件路径不可写 → `message.error`，不进入记录态。
12. 两个 tab 同时记录 → 各自独立文件、互不干扰。
