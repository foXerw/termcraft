# 预置命令：输出捕获 + 条件判定 + 分支中止（A）设计

日期：2026-06-19
范围：本 spec 只覆盖 A。B（reboot → 回连 → 续跑）是后续独立 spec。

## 背景

现有 `PresetEngine`（`src-tauri/src/preset/engine.rs`）只能 `write_to` 把命令发出去，**从不读取命令回显**，因此无法做结果判定与分支。`WaitCondition`（pattern/timeout/match_type）已在 `models.rs` 定义但执行时未使用。用户需要："对一组命令顺序执行、命令间设间隔、并能按回显做分支——例如 `ps` 找不到预期进程就中止整个任务。"

## 目标

- 让引擎能捕获每条命令发出后的输出，按条件匹配判定"成功/失败"。
- 失败时可配置中止或继续。
- 三协议（SSH/Telnet/Local）统一支持，显示链路（xterm）零影响。
- 顺带支持"单命令快速执行"（对当前活动终端跑一条命令）。

## 非目标

- 不做 reboot 后回连续跑（属 B）。
- 不做 pause/resume 的完整实现（保持现状返回未实现）。
- 不引入新的执行编排（如并发、跳转标签 Goto）；本次只做顺序 + 中止/继续。

## 架构：Output Tap（输出分流）

每个 handler 增加一个订阅槽：

```rust
output_tap: Arc<Mutex<Option<UnboundedSender<Vec<u8>>>>>
```

- handler 的字节转发处（`forward_task` / read loop）在把字节推给前端 Tauri Channel 的**同时**，复制一份发给订阅槽（仅当 `Some(sender)`）。
- SSH：`SSHClientHandler::data` → 当前已发 mpsc `output_tx`；在 `forward_task` 里 clone 一份到 tap。
- Telnet / Local：各自 read loop 里同样复制。
- `ConnectionManager` 暴露 `subscribe_output(conn_id) -> Option<UnboundedReceiver<Vec<u8>>>`：注册 sender、返回 receiver；并暴露 `unsubscribe_output(conn_id)`。`ConnectionEntry` 各变体持有其 handler 的 `output_tap` 引用，manager 经它设置 sender。

引擎执行 preset 时：
1. `manager.subscribe_output(conn_id)` 拿到 receiver。
2. 每条命令记录"水位"（已读字节数），`write_to` 后只看增量做匹配。
3. 执行结束（正常/中止/取消）`unsubscribe_output`，防止泄漏。

显示链路完全不变：xterm 仍收到全部输出。

## 数据模型（`preset/models.rs`）

复用已有 `WaitCondition` 并扩展；给 `CommandItem` 加 `on_fail`：

```rust
pub struct WaitCondition {
    pub pattern: String,
    pub timeout_ms: u64,
    pub match_type: MatchType,          // Exact | Contains | Regex（已存在）
    pub expect: WaitExpect,             // 新增：Found | NotFound，默认 Found
}
pub enum WaitExpect { Found, NotFound }

pub struct CommandItem {
    pub command: String,
    pub delay_ms: u64,
    pub wait_for: Option<WaitCondition>,
    pub on_fail: OnFail,                // 新增：Abort | Continue
    pub enabled: bool,
}
pub enum OnFail { Abort, Continue }
```

序列化保持 PascalCase。字段默认：`expect` 缺省 → Found；`on_fail` 缺省 → **有 `wait_for` 时 Abort，无 `wait_for` 时 Continue**（在加载/执行时补默认，而非依赖 serde 默认，以兼容旧数据）。

**场景映射**：`ps` 找进程 → `wait_for{pattern:"myproc", match_type:Contains, expect:Found, timeout_ms:3000}`，`on_fail:Abort`。3 秒内未看到 `myproc` → 失败 → 中止。

## 执行与分支流程（`engine.rs` 重构）

把 `execute_single`/`execute_batch`/`execute_loop` 的三份近乎重复循环收敛为统一逐步执行器 `run_commands`，外层模式只决定"跑几次"：

- `Single`：取第一条命令跑一次。
- `Batch`：全部命令跑一次。
- `Loop`：外层 `while` 包 `run_commands`，保留 `count`/`interval_ms`。

每条命令（`run_commands` 内）：

1. `check_cancelled` → 取消则发 `Cancelled` 并返回。
2. 发 `Running`（含 index/total）。
3. 记录订阅 receiver 当前水位（已累计字节数）。
4. `manager.write_to(conn_id, command + "\n")`。失败 → 命令失败，转第 6 步。
5. **判定**：
   - 无 `wait_for`：`sleep(delay_ms)` → 成功。
   - 有 `wait_for`：在 `timeout_ms` 内轮询订阅字节增量，按 `match_type` 匹配：
     - `expect=Found`：命中 → 成功；超时未命中 → 失败。
     - `expect=NotFound`：命中 → 失败；超时未命中 → 成功。
   - 命中后仍 `sleep(delay_ms)`（保留间隔语义）。
6. 结果处理：
   - 成功 → 下一条。
   - 失败 + `on_fail=Abort` → 发 `Failed`（带未匹配信息）并停止。
   - 失败 + `on_fail=Continue` → 发一条 `Running`/告警（记 captured snippet），继续下一条。

现有 `Batch{stop_on_error}` 退化为 `on_fail` 的 preset 级默认；保留字段向后兼容。

## 单命令快速执行（并入 A）

复用 `ExecutionMode::Single`：前端 Preset 面板对 Single 型 preset 给一个醒目"运行"按钮，target = **当前活动终端的 connectionId**。后端复用 `execute_preset` 命令（传 active tab 的 connectionId）。若没有活动终端，前端提示先连接。

## 错误处理与状态

- `write_to` 失败、订阅中途断开 → 视为命令失败，走 `on_fail`。
- `PresetExecutionStatus` 增加可选字段：
  - `command_succeeded: Option<bool>`
  - `message` 复用，写入如 `"第2条: 未匹配到 myproc"`
  - `captured_snippet: Option<String>`（最近一段回显，便于排错，长度受限如 512 字节）。
- 正则编译失败、超时等显式反映到状态，不静默吞掉。

## 边界情况

- **旧 preset 数据**：缺 `expect`/`on_fail` 字段的 JSON 加载时按规则补默认（Found / 上文规则），不破坏旧数据。
- **无 `wait_for`**：仅 `delay_ms` 间隔，行为与现状一致。
- **匹配边界**：增量匹配仅看该命令发出后的字节（水位之后），避免误命中上一条命令的回显。
- **超时与延迟关系**：`timeout_ms` 是"等条件"的窗口；`delay_ms` 是命中/无条件后的"额外等待"。两者独立。
- **正则**：预编译一次；非法正则在执行该命令时直接判失败 + 告警（而非 preset 启动即崩）。
- **退订**：preset 结束（任何原因）必须 `unsubscribe_output`，用 RAII/Drop 或 try-finally 保证。

## 测试 / 验证

1. `cargo check`；`tsc --noEmit`。
2. 后端单测（或内联验证）：对注入固定字节序列的 mock ConnectionManager，验证 Found 命中、NotFound、超时、Abort 中止、Continue 继续 五条分支。
3. 端到端（`npm run tauri dev`）：
   - 真实 SSH 连接，跑 "ps 找进程" preset（expect=Found, on_fail=Abort），故意填一个不存在进程名 → 确认中止、状态显示"未匹配"。
   - 同命令换 expect=NotFound / on_fail=Continue → 确认不中止、继续。
   - 单命令 preset 点"运行" → 在当前终端执行。
   - 多条命令的 Batch → 间隔生效。
