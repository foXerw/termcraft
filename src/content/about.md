# 关于 TermCraft

TermCraft 是一款基于 Tauri v2 的桌面终端客户端，支持 SSH、Telnet 与本地 shell（cmd / bash），界面中文本地化。

## 主要特性

- **多连接终端**：SSH（russh）、Telnet、本地 shell（portable_pty），每连接独立标签页。
- **预设命令**：变量替换（`{{name}}`）、单条 / 批量 / 循环执行、命令间条件等待（匹配输出后继续）、命令失败时中止或继续、输出捕获与匹配；支持预设的导入与导出。
- **终端日志记录**：对单个终端记录输出与输入到日志文件，关闭终端自动停止；终端底部显示日志路径，点击可用默认应用打开。
- **连接可达性检测**：列表显示 SSH/Telnet 连接的在线状态与延迟。

## 安全说明

- 连接密码、SSH 密钥口令**不会以明文写入 `connections.json`**，磁盘上只保留空标记。
- 实际密钥存于操作系统凭据库：
  - Windows：Credential Manager
  - macOS：Keychain
  - Linux：Secret Service（D-Bus）
- 经 `keyring` crate 访问，凭据项以 `(com.termcraft.app, <连接ID>:<字段>)` 为键。
- 删除连接时会一并清理对应的凭据项。

## 技术栈

- Tauri v2（Rust 后端 + React 前端）
- xterm.js 5（终端渲染，WebGL addon）
- Ant Design 5、Zustand 4

## 数据位置

配置以 JSON 原子写入用户数据目录下的 `termcraft/`：

- Windows：`C:\Users\<用户>\AppData\Roaming\termcraft\`
- macOS：`~/Library/Application Support/termcraft/`
- Linux：`~/.local/share/termcraft/`

包含 `connections.json`、`presets.json`、`groups.json`、`schedules.json`、`settings.json`。
