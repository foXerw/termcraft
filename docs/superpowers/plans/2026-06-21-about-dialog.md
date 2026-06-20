# 关于（About）说明页 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 顶部栏最右加「关于」按钮，点击打开 Modal，用 react-markdown 渲染 `about.md` 说明内容（应用特性 + keyring 安全说明等）。

**Architecture:** AntD Modal 沿用 app 的暗色 ConfigProvider 主题；内容是 `src/content/about.md`，经 Vite `?raw` 打包导入，由 `react-markdown` 渲染；`.about-markdown` CSS 适配暗色 token。打开状态走 `appStore`（与 `connectionFormOpen` 同模式）。

**Tech Stack:** React 18 + Ant Design 5 + Zustand + `react-markdown` 10。

## Global Constraints

- 项目无测试/无 lint/无测试基建（CLAUDE.md）。用 `npm run build`（tsc+vite）通过 + 手动验证替代 TDD。
- UI 文案中文（zh-CN）。
- 路径别名 `@/*` → `src/*`。
- 内容用 Vite `?raw` 导入（`import aboutMd from "../content/about.md?raw"`），打包进 bundle，无需运行时 fetch。
- 不加 remark-gfm（基础标题/列表/行内代码/链接够用）。
- react-markdown v9+ 用 children 传 markdown 字符串：`<ReactMarkdown>{aboutMd}</ReactMarkdown>`。

参考 spec：`docs/superpowers/specs/2026-06-21-about-dialog-design.md`

---

## File Structure

| 文件 | 责任 | 动作 |
|------|------|------|
| `package.json` | JS 依赖 | 修改：加 `react-markdown` |
| `src/content/about.md` | 说明内容（中文 Markdown） | 新建 |
| `src/vite-env.d.ts` | 声明 `*?raw` 模块类型 | 新建 |
| `src/styles/global.css` | `.about-markdown` 暗色主题样式 | 修改 |
| `src/stores/appStore.ts` | `aboutOpen` + actions | 修改 |
| `src/components/AboutDialog.tsx` | Modal + react-markdown 渲染 | 新建 |
| `src/components/layout/AppLayout.tsx` | 顶部栏「关于」按钮 + 挂载 Modal | 修改 |

---

### Task 1: 加 react-markdown 依赖 + about.md 内容 + .about-markdown 样式

**Files:**
- Modify: `package.json`
- Create: `src/content/about.md`
- Create: `src/vite-env.d.ts`
- Modify: `src/styles/global.css`

**Interfaces:**
- Produces: `src/content/about.md`（可被 `?raw` 导入为字符串）；`src/vite-env.d.ts`（声明 `*?raw` 模块类型，让 tsc 认 `import ... from "...md?raw"`）；`.about-markdown` CSS 类（供 AboutDialog 使用）。

- [ ] **Step 1: 装 react-markdown**

Run: `npm install react-markdown`
Expected: `package.json` 出现 `"react-markdown": "^10"` 之类。

- [ ] **Step 2: 创建 vite-env.d.ts（声明 ?raw 模块类型）**

项目当前没有 `src/vite-env.d.ts`，tsconfig 也未引入 `vite/client` 类型，因此 `import x from "...md?raw"` 会被 tsc 报 "Cannot find module"。创建 `src/vite-env.d.ts`：

```ts
/// <reference types="vite/client" />
```

`vite/client` 声明了 `declare module '*?raw' { const src: string; export default src }`，覆盖 `*.md?raw`。

- [ ] **Step 3: 创建 about.md**

创建 `src/content/about.md`：

```markdown
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
```

- [ ] **Step 4: 加 .about-markdown 样式**

在 `src/styles/global.css` 末尾追加：

```css
.about-markdown h1 {
  font-size: 18px;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 12px;
}

.about-markdown h2 {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin-top: 16px;
  margin-bottom: 8px;
}

.about-markdown h3 {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  margin-top: 12px;
  margin-bottom: 6px;
}

.about-markdown p {
  color: var(--text-primary);
  line-height: 1.6;
  margin-bottom: 8px;
}

.about-markdown ul {
  color: var(--text-primary);
  padding-left: 20px;
  margin-bottom: 8px;
}

.about-markdown li {
  line-height: 1.6;
  margin-bottom: 4px;
}

.about-markdown code {
  background-color: var(--bg-tertiary);
  color: var(--warning-color);
  padding: 1px 5px;
  border-radius: 3px;
  font-family: Consolas, 'Courier New', monospace;
  font-size: 12px;
}

.about-markdown a {
  color: var(--accent-color);
  text-decoration: none;
}

.about-markdown a:hover {
  text-decoration: underline;
}
```

- [ ] **Step 5: 类型检查**

Run: `npm run build`
Expected: tsc + vite 通过（about.md 尚未被导入，但 `?raw` 导入在 Task 3 才接；本步确认 react-markdown 装好、vite-env.d.ts 生效、CSS 无语法错）。

- [ ] **Step 6: Commit**

```bash
git add package.json package-lock.json src/content/about.md src/vite-env.d.ts src/styles/global.css
git commit -m "feat(about): add react-markdown dep, about.md content, markdown styles"
```

---

### Task 2: appStore 加 aboutOpen 状态

**Files:**
- Modify: `src/stores/appStore.ts`

**Interfaces:**
- Produces: `aboutOpen: boolean`；`openAbout: () => void`；`closeAbout: () => void`。

- [ ] **Step 1: 加状态字段与 actions**

在 `src/stores/appStore.ts` 的 `AppState` interface，`connectionFormOpen` / `editingConfig` 附近加：

```ts
  // About dialog
  aboutOpen: boolean;
```

在 actions 区（`openConnectionForm`/`closeConnectionForm` 声明附近）加：

```ts
  openAbout: () => void;
  closeAbout: () => void;
```

在 `create<AppState>((set) => ({ ... }))` 初始状态加（`connectionFormOpen: false` 附近）：

```ts
  aboutOpen: false,
```

在 actions 实现区（`closeConnectionForm` 附近）加：

```ts
  openAbout: () => set({ aboutOpen: true }),

  closeAbout: () => set({ aboutOpen: false }),
```

- [ ] **Step 2: 类型检查**

Run: `npm run build`
Expected: tsc 通过（本文件无新错误）。检查 `mcp__ide__getDiagnostics` on appStore.ts 为空。

- [ ] **Step 3: Commit**

```bash
git add src/stores/appStore.ts
git commit -m "feat(about): add aboutOpen state to appStore"
```

---

### Task 3: AboutDialog 组件 + AppLayout 按钮挂载

**Files:**
- Create: `src/components/AboutDialog.tsx`
- Modify: `src/components/layout/AppLayout.tsx`

**Interfaces:**
- Consumes: Task 1 的 `about.md`（`?raw`）、`.about-markdown` 样式、`react-markdown`；Task 2 的 `aboutOpen`/`closeAbout`。

- [ ] **Step 1: 创建 AboutDialog.tsx**

创建 `src/components/AboutDialog.tsx`：

```tsx
import React from "react";
import { Modal, Typography, Button } from "antd";
import ReactMarkdown from "react-markdown";
import aboutMd from "../content/about.md?raw";
import { useAppStore } from "../stores/appStore";

const AboutDialog: React.FC = () => {
  const aboutOpen = useAppStore((s) => s.aboutOpen);
  const closeAbout = useAppStore((s) => s.closeAbout);

  return (
    <Modal
      title="关于 TermCraft"
      open={aboutOpen}
      onCancel={closeAbout}
      width={640}
      footer={[<Button key="close" onClick={closeAbout}>关闭</Button>]}
    >
      <Typography>
        <div className="about-markdown">
          <ReactMarkdown>{aboutMd}</ReactMarkdown>
        </div>
      </Typography>
    </Modal>
  );
};

export default AboutDialog;
```

- [ ] **Step 2: AppLayout 加按钮并挂载 Modal**

在 `src/components/layout/AppLayout.tsx` 顶部 import 区加：

```tsx
import { InfoCircleOutlined } from "@ant-design/icons";
import AboutDialog from "../AboutDialog";
```

在组件内取 `aboutOpen`/`openAbout`（在现有 `useAppStore` 选择器附近）：

```tsx
  const aboutOpen = useAppStore((s) => s.aboutOpen);
  const openAbout = useAppStore((s) => s.openAbout);
```

在顶部栏（现有 `+` 新建连接 `<Button>` 之后）加「关于」按钮（`marginLeft: "auto"` 推到最右）：

```tsx
            <Button
              type="text"
              icon={<InfoCircleOutlined />}
              size="small"
              style={{ marginLeft: "auto" }}
              onClick={openAbout}
            >
              关于
            </Button>
```

在 `<ConnectionForm ... />` 之后挂载 AboutDialog：

```tsx
      <AboutDialog />
```

- [ ] **Step 3: 类型检查**

Run: `npm run build`
Expected: tsc + vite 通过。检查 `mcp__ide__getDiagnostics` on AboutDialog.tsx 与 AppLayout.tsx 为空。

- [ ] **Step 4: Commit**

```bash
git add src/components/AboutDialog.tsx src/components/layout/AppLayout.tsx
git commit -m "feat(about): add AboutDialog modal and wire top-bar entry"
```

---

### Task 4: 集成手动验证

**Files:** 无（运行验证）

- [ ] **Step 1: 启动 dev**

Run: `npm run tauri dev`
Expected: 应用启动，无编译/运行错误。

- [ ] **Step 2: 入口可见**

顶部栏最右出现「关于」按钮（图标 + 文字），与 `+` 分开、右对齐。

- [ ] **Step 3: 打开 Modal**

点击「关于」→ Modal 打开，标题「关于 TermCraft」，宽度约 640px。

- [ ] **Step 4: 内容渲染**

Markdown 正确渲染：一级标题「关于 TermCraft」、二级标题（主要特性/安全说明/技术栈/数据位置）、列表项、行内代码（如 `{{name}}`、`connections.json`）样式符合暗色主题（行内代码用 `--bg-tertiary` 背景 + `--warning-color` 字色）。

- [ ] **Step 5: 安全说明准确**

「安全说明」章节正确描述：密码/口令不存明文、存于 OS 凭据库（Windows Credential Manager / macOS Keychain / Linux Secret Service）、经 keyring、删除连接清理凭据。

- [ ] **Step 6: 数据位置准确**

「数据位置」章节路径正确（Windows `AppData\Roaming\termcraft\`），列出 5 个 json 文件。

- [ ] **Step 7: 关闭**

点「关闭」按钮 / 点遮罩 / Esc → Modal 关闭。

---

## Self-Review 记录

- **Spec 覆盖**：入口(Task 3)、Modal(Task 3)、状态(Task 2)、内容 about.md(Task 1)、样式(Task 1)、依赖(Task 1)、数据位置/安全说明内容(Task 1 + Task 4 验证) 全覆盖。
- **占位符**：无 TBD/TODO；每步含实际代码或确切命令，about.md 内容完整写出。
- **类型一致性**：`aboutOpen: boolean` + `openAbout()`/`closeAbout()` 在 Task 2 定义、Task 3 消费；`import aboutMd from "../content/about.md?raw"` 路径（components → content 是 `../content`）正确；react-markdown v9+/10 用 children 字符串 API。
- **与 CLAUDE.md 一致**：无测试基建，用编译+手动验证替代 TDD。
