# 连接可达性状态（Reachability Status）设计

日期：2026-06-19

## 背景

连接列表（`Sidebar` → `ConnectionCard`）目前只显示名称与类型，用户无从得知一台主机当前是否在线。希望在每条连接旁展示实时"可达/不可达"状态。要求**优雅而非无脑持续 ping**：不每秒打、不对宕机主机狂打。

## 目标

- 每个有 host 的连接在列表旁显示可达性状态点（绿/红/灰）。
- 后端统一调度探测，错峰 + 失败退避，避免对宕机主机持续猛击。
- 状态变化即时推送到前端；前端只渲染、不轮询。

## 非目标（v1 不做）

- 不做窗口隐藏/失焦时暂停（保持简单，后续可优化）。
- 不做 ICMP 探测（需管理员权限，桌面应用不适用）。
- 不做跨主机批量并发上限的精细控制（目标数量有限，靠错峰即可）。

## 探测方式

对连接的 `host:port` 发起一次**带超时（1.5s）的 TCP connect**：

- 连接建立成功 → 可达，记录耗时为延迟（ms）。
- 超时或被拒 → 不可达。
- 复用 `src-tauri/src/connection/telnet.rs` 中 `TcpStream` 的连接模式；新探测代码用 `tokio::net::TcpStream::connect` + `tokio::time::timeout`。

无需任何管理员权限。

## 架构

### 后端

新建模块 `src-tauri/src/reachability/`（`mod.rs` 暴露接口）：

- `ReachState`：`{ status: Status, latency_ms: Option<u32>, last_checked: Option<DateTime> }`，其中 `Status ∈ {Checking, Reachable, Down, Unknown}`。
- 状态表：`Arc<Mutex<HashMap<String, ReachTarget>>>`，`ReachTarget` 含 `host`、`port`、`interval`（当前间隔）、`next_check_at`、上一次 `ReachState`。
- 调度循环：一个 tokio 任务（在 `lib.rs::run()` 的 `tauri::Builder` setup 钩子中 `tokio::spawn`），每 ~1s 唤醒一次，扫描状态表：
  - 对 `next_check_at <= now` 的目标发起探测（await，单次）。
  - 按 `next_check_at` 错峰——目标注册时 `next_check_at = now + index * spread`（如 spread=1.5s），避免同时打。
  - 成功：`interval = BASE`（30s）；失败：`interval = min(interval*2, CAP)`，指数退避，封顶 300s。
  - 探测完成更新 `last_checked` 与延迟，重算 `next_check_at = now + interval`。
  - **仅当 `status` 发生变化时**，通过 `app_handle.emit("connection_status", payload)` 推送，payload 含 `conn_id`、`status`、`latency_ms`、`last_checked`。
- 注册/注销命令（Tauri command，供前端调用）：
  - `set_reachability_targets(targets: Vec<(conn_id, host, port)>)`：替换当前目标集合。新增的目标 `next_check_at` 设为"尽快"以触发首次探测。
  - 隐式：状态表挂在 `AppState` 上（`Arc<ReachabilityService>`），调度循环持有同一 `Arc`。

### 前端

- `connectionStore` 增加 `statusMap: Record<conn_id, ReachState>`。
- `AppLayout`（或顶层）启动时 `listen("connection_status", ...)`，收到事件更新 `statusMap`。
- 加载/变更配置后（`load_connection_configs`、新增、删除、编辑 host/port）调用 `set_reachability_targets` 重新注册（传当前所有有 host 的连接）。
- `ConnectionCard` 在 `LinkOutlined` 图标处替换为状态点：
  - 🟢 可达（`Reachable`）：绿色圆点，tooltip `"{latency}ms · {相对时间}"`。
  - 🔴 不通（`Down`）：红色圆点。
  - ⚫ 灰：`Checking` / `Unknown` / LocalShell（无 host）。

## 数据流

```
Sidebar mount → load_connection_configs → set_reachability_targets(...)
                                          → 后端状态表更新
后端调度循环 → 探测 → status 变化 → emit("connection_status")
                                          → 前端 listen → statusMap 更新 → 圆点变色
```

## 边界情况

- **LocalShell / 无 host**：前端不把它放进 targets；`ConnectionCard` 对无 host 的连接恒显示灰点。
- **超时**：1.5s 内未连上即判 Down。
- **配置编辑（host/port 变）**：保存后再次调用 `set_reachability_targets`，该目标会被重置间隔、立即重探。
- **删除连接**：`set_reachability_targets` 后该 id 不再在集合中，后端移除目标，停止探测。
- **重复 emit**：仅状态翻转才 emit，避免刷屏与冗余渲染。
- **并发**：状态表用 `Mutex`；探测本身在调度循环内串行 await（目标数量有限，可接受）。

## 测试 / 验证

1. `cargo check` 编译通过；`tsc --noEmit` 类型检查通过。
2. `npm run tauri dev`：
   - 列表中可达主机显示绿点，hover 看到延迟。
   - 关掉某主机 → 等待退避周期后变红（可临时把 BASE 调小加速观察）。
   - 新增/编辑/删除连接后状态点随之更新。
   - LocalShell 恒为灰点。
3. 观察后端日志确认错峰与退避生效（不会对宕机主机每秒探测）。
