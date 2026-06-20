# 关于（About）说明页设计

日期：2026-06-21
状态：已确认，待实现

## 背景

TermCraft 顶部菜单栏目前只有侧栏开关、TabBar、新建连接按钮，缺少介绍应用特性与实现细节的入口。用户希望加一个「关于」入口，打开说明页，介绍应用特点和安全细节（如连接密码经 keyring 存于 OS 凭据库）。

## 目标

- 顶部栏最右端加「关于」按钮，点击打开 Modal。
- Modal 用 Markdown 渲染说明内容，涵盖应用简介、主要特性、安全说明、技术栈、数据位置。
- 沿用现有 Modal/store 模式（与 SettingsDialog、ConnectionForm 一致）。

## 非目标

- 不做版本检测/更新检查。
- 不做可编辑内容/管理后台。
- 不加 remark-gfm 等额外 markdown 插件（基础标题/列表/代码/链接够用，YAGNI）。
- 不做多语言切换（文案中文，与 app 一致）。

## 设计

### 入口（`src/components/layout/AppLayout.tsx`）

顶部栏（现有 `display:flex; align-items:center` 的 div）在 `+` 新建连接按钮之后加一个「关于」按钮：
- AntD `Button type="text" size="small"`，图标 `InfoCircleOutlined` + 文字「关于」。
- `style={{ marginLeft: "auto" }}` 把它推到顶部栏最右端（与 `+` 分开，属于不同类别）。
- `onClick` → `openAbout()`。

### 状态（`src/stores/appStore.ts`）

仿 `connectionFormOpen` 模式：
- `aboutOpen: boolean`（初始 false）。
- `openAbout: () => void` → `set({ aboutOpen: true })`。
- `closeAbout: () => void` → `set({ aboutOpen: false })`。

### Modal（`src/components/AboutDialog.tsx`，新建）

- AntD `Modal`：`title="关于 TermCraft"`，`open={aboutOpen}`，`onCancel={closeAbout}`，`footer={[<Button onClick={closeAbout}>关闭</Button>]}`，`width={640}`。
- 内容区：`<Typography><div className="about-markdown"><ReactMarkdown>{aboutMd}</ReactMarkdown></div></Typography>`。
- Markdown 内容用 Vite `?raw` 导入：`import aboutMd from "../content/about.md?raw";`（打包进 bundle，无需运行时 fetch）。

### 内容（`src/content/about.md`，新建）

中文 Markdown，大纲：

1. **应用简介**：TermCraft 是基于 Tauri v2 的桌面终端/SSH/Telnet 客户端。
2. **主要特性**：
   - SSH / Telnet / 本地 shell（cmd/bash）多连接
   - 预设命令：变量替换、单/批/循环执行、条件等待、输出捕获与匹配；支持导入导出
   - 终端日志记录：每终端独立记录输出+输入到文件，关闭自动停止
   - 连接可达性检测：列表显示 SSH/Telnet 连接的在线状态与延迟
3. **安全说明**（重点）：
   - 连接密码、SSH 密钥口令**不存于 `connections.json` 明文**，仅保留空标记。
   - 实际存于操作系统凭据库：Windows Credential Manager / macOS Keychain / Linux Secret Service（经 `keyring` crate）。
   - 删除连接时一并清理对应凭据项。
4. **技术栈**：Tauri v2（Rust 后端 + React 前端）、xterm.js、Ant Design、Zustand。
5. **数据位置**：用户数据目录下 `termcraft/`（Windows: `C:\Users\<用户>\AppData\Roaming\termcraft\`），含 `connections.json`、`presets.json`、`groups.json`、`schedules.json`、`settings.json`。

### 样式（`src/styles/global.css`）

新增 `.about-markdown` 作用域样式，适配暗色主题 token：
- `h1/h2/h3`：`--text-primary`，合适字重与上下边距。
- `p`、`ul/li`：`--text-primary`，行高与段落间距。
- `code`（行内）：`--bg-tertiary` 背景、`--warning-color` 或次色字、圆角 padding。
- `a`：`--accent-color`，下划线 hover。
- `ul`：左侧 padding/list-style。
- Modal 内容区自带滚动（AntD Modal body 默认 maxHeight）。

### 依赖

- 新增 `react-markdown`（不加 remark-gfm）。

## 错误处理与边界

- 内容是静态打包的 `?raw` 字符串，无运行时加载失败可能。
- Modal 关闭即卸载（`destroyOnClose`），无状态泄漏。
- 不涉及权限/IPC。

## 涉及文件

| 文件 | 改动 |
|------|------|
| `src/content/about.md` | 新建：说明内容（中文 Markdown） |
| `src/components/AboutDialog.tsx` | 新建：Modal + react-markdown 渲染 |
| `src/stores/appStore.ts` | 加 `aboutOpen` + `openAbout`/`closeAbout` |
| `src/components/layout/AppLayout.tsx` | 顶部栏加「关于」按钮 + 挂载 AboutDialog |
| `src/styles/global.css` | 加 `.about-markdown` 暗色主题样式 |
| `package.json` | 加 `react-markdown` |

## 测试要点

手动验证（项目无测试基建）：
1. 顶部栏最右出现「关于」按钮。
2. 点击 → Modal 打开，标题「关于 TermCraft」，Markdown 正确渲染（标题/列表/行内代码/链接样式正常，暗色主题）。
3. 「关闭」按钮或点遮罩/Esc 关闭 Modal。
4. 安全说明章节提到 keyring / OS 凭据库，无明文。
5. 数据位置章节路径正确（Windows AppData\Roaming\termcraft）。
