# TermCraft

一款高度可定制的桌面终端 / SSH / Telnet 客户端，基于 **Tauri v2**（Rust 后端 + React 前端）构建。界面语言为简体中文（zh-CN）。

> 当前版本：v0.1.0（早期开发阶段，部分功能尚为占位或待实现）

## ✨ 功能特性

- **多协议支持**
  - **SSH**：基于 `russh`，支持 PTY 伪终端、密钥/密码认证
  - **Telnet**：基于 `tokio` 原始 TCP 流
  - **本地 Shell**：基于 `portable_pty` 的本地终端
  - **串口（Serial）**：基于 `serialport`，支持端口枚举下拉与波特率/数据位/校验/停止位/流控配置
- **流畅的终端渲染**：xterm.js 5 + WebGL addon，硬件加速渲染
- **连接管理**：分组管理连接配置，支持新增/编辑/删除
- **预设命令引擎（PresetEngine）**
  - 变量替换语法 `{{name}}`
  - 单条 / 批量 / 循环三种执行模式
  - 运行中可取消
- **预设导入/导出**：支持将预设导出为文件、从文件导入（带逐项冲突解决）与模板文件解析
- **终端会话日志**：每个终端可单独启动/停止日志记录，日志路径栏可点击打开，自动剥离 ANSI 转义以保证纯文本可读
- **关于对话框**：应用信息弹窗，正文由 Markdown 渲染
- **本地数据持久化**：连接、预设、分组、计划任务、设置等以 JSON 文件原子化写入（写 `.tmp` 再重命名）；SSH/Telnet 密码与密钥口令存入系统凭据库（Windows Credential Manager / macOS Keychain / Linux Secret Service），JSON 中仅保留空占位符
- **现代化 UI**：React 18 + Ant Design 5 + Zustand 状态管理

## 🛠 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri v2（含 `tauri-plugin-shell` / `dialog` / `opener`） |
| 后端 | Rust、Tokio、russh、portable-pty、serialport、keyring |
| 前端框架 | React 18、TypeScript |
| UI 组件库 | Ant Design 5 |
| 状态管理 | Zustand 4 |
| 终端渲染 | xterm.js 5（+ WebGl / Fit addon） |
| 构建工具 | Vite 5 |

## 📦 环境要求

在开始之前，请确保已安装以下工具：

- **Node.js**（建议 18+）
- **Rust**（stable 工具链）
- **Tauri v2 前置依赖**：参考 [Tauri 官方文档](https://tauri.app/start/prerequisites/)（Windows 需 Microsoft C++ Build Tools 与 WebView2）

## 🚀 快速开始

### 安装依赖

```bash
cd termcraft
npm install
```

### 开发模式（热重载）

```bash
npm run tauri dev
```

### 生产构建

```bash
npm run tauri build      # 编译并打包为可安装应用
```

### 仅前端开发（无 Rust 后端）

```bash
npm run dev              # 启动 Vite 开发服务器（端口 1420）
npm run build            # 仅构建前端（tsc && vite build）
```

> ⚠️ 本项目暂未配置测试、Lint 或 Format 命令。

## 🏗 项目结构

```
termcraft/
├── src/                          # 前端源码
│   ├── main.tsx → App.tsx        # UI 入口
│   ├── components/               # React 组件（connection / layout / preset / settings / terminal / AboutDialog）
│   ├── content/                  # 静态正文（如 about.md）
│   ├── stores/                   # Zustand 状态（appStore / connectionStore / presetStore）
│   ├── hooks/
│   ├── types/
│   └── styles/
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs → lib.rs      # 应用启动入口
│   │   ├── ipc_commands.rs       # Tauri 命令处理器
│   │   ├── config/               # 数据模型与 JSON 持久化
│   │   │   ├── models.rs
│   │   │   └── store.rs
│   │   ├── connection/           # 协议实现（统一 ConnHandler trait）
│   │   │   ├── ssh.rs
│   │   │   ├── telnet.rs
│   │   │   ├── local.rs
│   │   │   ├── serial.rs        # 串口（serialport）
│   │   │   ├── manager.rs        # ConnectionManager（trait object）
│   │   │   └── logger.rs         # 终端会话日志
│   │   ├── preset/               # 预设引擎
│   │   │   ├── engine.rs
│   │   │   ├── scheduler.rs
│   │   │   ├── template.rs
│   │   │   └── models.rs
│   │   ├── security/credentials.rs  # 系统凭据库读写
│   │   └── reachability/mod.rs
│   ├── tauri.conf.json           # Tauri 配置
│   └── Cargo.toml
├── package.json
└── vite.config.ts
```

## 🧩 架构要点

### 前后端通信（IPC）

所有前后端通信均通过 Tauri 的 IPC 桥接：

- **流式数据（终端输出）**：使用 `Tauri Channel`。前端创建一个 `Channel`，通过 `invoke()` 传递给 Rust 命令，再通过 `channel.onmessage` 将数据写入 `xterm.write()`。实例存储于 `appStore.channels`。
- **命令式请求**：使用标准 `invoke()` 调用，用于 CRUD、预设执行等。

### 后端状态

- `AppState` 持有 `ConnectionManager` 与 `Mutex<PresetEngine>`，通过 Tauri `.manage()` 注入。
- `ConnectionManager` 使用 `HashMap<String, Arc<Mutex<Box<dyn ConnHandler + Send + Sync>>>>`，各协议 handler（SSH/Telnet/LocalShell/Serial）实现统一的 `ConnHandler` trait；新增协议只需一个 `impl` 块，无需改 manager。

### 数据存储位置

配置文件以 JSON 形式原子化保存于 `dirs::data_dir()/termcraft/` 目录下：

`connections.json`、`presets.json`、`groups.json`、`schedules.json`、`settings.json`

## 🗺 功能状态

部分组件当前为占位或待实现（TODO）：

- SSH Agent 认证：返回 “未实现” 错误
- 服务器密钥校验：`check_server_key` 暂时总是返回 `true`
- 预设暂停/恢复：未完全实现
- 计划任务（PresetScheduler）：仅支持 Interval 模式，Cron 为 TODO
- 等待条件（WaitCondition）：已定义但未在执行中使用

## 📄 许可证

本项目暂未指定开源许可证。如需使用或贡献，请联系作者。
