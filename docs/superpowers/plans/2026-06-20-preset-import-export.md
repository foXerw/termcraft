# 预设命令导入导出 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给预设面板加导入/导出能力——导出单个或全部预设到 `.tc-presets.json` 文件，从文件导入并逐项解决 ID 冲突。

**Architecture:** 复用后端已有的 `template::PresetTemplate` 类型与 `export_template` 组装逻辑。新增 `parse_template`（只解析+版本校验，不 apply）。新增两个 IPC：`export_presets_to_file(path, preset_ids)` 写文件、`parse_template_file(path)` 只读返回。导入的冲突解决与 apply 在前端 `PresetImportDialog` 组件里做，复用现有 `save_preset`/`save_preset_group`。文件选择用 `tauri-plugin-dialog` 原生 open/save 对话框。移除从未接线的旧 `export_template`/`import_template` IPC。

**Tech Stack:** Rust + Tauri v2 + `tauri-plugin-dialog`；React + Ant Design 5 + Zustand。

## Global Constraints

- 项目无测试/无 lint 命令（CLAUDE.md 明示）。因此本计划用「`cargo build` / `npm run build` 编译通过 + 手动验证」替代单元测试步骤，遵循 CLAUDE.md 而非 TDD 默认。
- UI 文案中文（zh-CN）。
- 路径别名 `@/*` → `src/*`。
- 所有文件 IO 留在 Rust；前端不持有 fs 权限，仅用 dialog 拿路径。
- 复用现有 `PresetTemplate`（`version: "1.0"` 字符串，无 `format` 字段），不新建类型。
- 旧 IPC `export_template`/`import_template` 在 Task 3 一并移除。

参考 spec：`docs/superpowers/specs/2026-06-20-preset-import-export-design.md`

---

## File Structure

| 文件 | 责任 | 动作 |
|------|------|------|
| `src-tauri/Cargo.toml` | Rust 依赖 | 修改：加 `tauri-plugin-dialog` |
| `src-tauri/src/lib.rs` | Tauri builder | 修改：注册 dialog 插件；移除旧命令注册 |
| `src-tauri/capabilities/default.json` | 权限 | 修改：加 `dialog:default` |
| `src-tauri/src/preset/template.rs` | 模板序列化/解析 | 修改：加 `parse_template`；删 `import_template` |
| `src-tauri/src/ipc_commands.rs` | IPC 命令 | 修改：加两个命令、删两个旧命令 |
| `package.json` | JS 依赖 | 修改：加 `@tauri-apps/plugin-dialog` |
| `src/types/preset.ts` | 前端类型 | 修改：加 `PresetTemplate` |
| `src/components/preset/PresetImportDialog.tsx` | 冲突解决 Modal | 新建 |
| `src/components/preset/PresetPanel.tsx` | 面板 UI | 修改：加按钮、接导出/导入 |

---

### Task 1: 加 `tauri-plugin-dialog` 依赖与权限

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`

**Interfaces:**
- Produces: Tauri 应用注册了 dialog 插件，前端可 `import { open, save } from "@tauri-apps/plugin-dialog"`（JS 依赖在 Task 4 装，本任务只做 Rust 侧）。

- [ ] **Step 1: Cargo.toml 加依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 末尾（`keyring` 行之后）加：

```toml
tauri-plugin-dialog = "2"
```

- [ ] **Step 2: lib.rs 注册插件**

在 `src-tauri/src/lib.rs` 的 `tauri::Builder::default()` 链中，`.plugin(tauri_plugin_shell::init())` 之后加一行：

```rust
        .plugin(tauri_plugin_dialog::init())
```

- [ ] **Step 3: capabilities 加权限**

在 `src-tauri/capabilities/default.json` 的 `permissions` 数组里加 `"dialog:default"`。改后大致：

```json
  "permissions": [
    "core:default",
    "shell:allow-open",
    "dialog:default",
    {
      "identifier": "shell:allow-execute",
      "allow": [
        { "cmd": "", "name": "", "sidecar": false }
      ]
    }
  ]
```

- [ ] **Step 4: 编译验证**

Run: `cd src-tauri && cargo build`
Expected: 编译通过，无 dialog 相关错误。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "feat(preset): add tauri-plugin-dialog for file open/save dialogs"
```

---

### Task 2: 后端 template 模块加 `parse_template`、删 `import_template`

**Files:**
- Modify: `src-tauri/src/preset/template.rs`

**Interfaces:**
- Produces: `pub fn parse_template(json: &str) -> Result<PresetTemplate, AppError>`（解析+版本校验，不 apply）。
- 移除: `pub fn import_template(...)`（被前端逐项决策取代）。
- 保留: `PresetTemplate` 结构、`export_template(presets, groups) -> Result<String, AppError>` 不动。

- [ ] **Step 1: 用 `parse_template` 替换 `import_template`**

在 `src-tauri/src/preset/template.rs` 中，删除整个 `import_template` 函数（从 `/// Import presets from a JSON template string` 注释到其函数体结束），替换为：

```rust
/// 解析模板字符串并校验版本，不写任何文件、不 apply。
/// 由前端拿到返回的 PresetTemplate 后弹冲突解决 Modal，再用
/// save_preset / save_preset_group 逐项应用。
pub fn parse_template(json: &str) -> Result<PresetTemplate, AppError> {
    let template: PresetTemplate = serde_json::from_str(json)
        .map_err(|e| AppError::Preset(format!("无法解析预设文件: {}", e)))?;
    if template.version != "1.0" {
        return Err(AppError::Preset(format!("不支持的预设文件版本: {}", template.version)));
    }
    Ok(template)
}
```

- [ ] **Step 2: 编译验证（预期 import_template 的旧调用方报错）**

Run: `cd src-tauri && cargo build`
Expected: 在 `ipc_commands.rs` 报错 `cannot find function import_template`（因为 Task 3 还没改 IPC）。这是预期的——Task 3 会修掉。先不提交，继续 Task 3。

> 说明：本任务不单独提交，与 Task 3 一起提交，避免中间态编译失败。

---

### Task 3: 后端 IPC 命令——加两个、删两个、改注册

**Files:**
- Modify: `src-tauri/src/ipc_commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub async fn export_presets_to_file(path: String, preset_ids: Vec<String>) -> Result<(), String>`（`preset_ids` 空=全量）
  - `pub async fn parse_template_file(path: String) -> Result<PresetTemplate, String>`（只读返回）
- 移除: `export_template`、`import_template` 两个 IPC 命令。
- 依赖 Task 2 的 `parse_template`。

- [ ] **Step 1: 删除旧的两个 IPC 命令**

在 `src-tauri/src/ipc_commands.rs` 中，删除 `// === Template Commands ===` 注释下的整个 `export_template` 命令和 `import_template` 命令（从 `#[tauri::command]\npub async fn export_template(...)` 到 `import_template` 函数末尾的 `Ok(new_presets.iter()...)` 及其闭合 `}`）。

- [ ] **Step 2: 加 `export_presets_to_file` 命令**

在删掉旧命令的位置加：

```rust
// === Preset Import/Export ===

#[tauri::command]
pub async fn export_presets_to_file(path: String, preset_ids: Vec<String>) -> Result<(), String> {
    let presets = store::load_presets().map_err(|e| e.to_string())?;
    let groups = store::load_preset_groups().map_err(|e| e.to_string())?;

    // preset_ids 为空 => 导出全部
    let selected_presets: Vec<Preset> = if preset_ids.is_empty() {
        presets
    } else {
        presets.iter().filter(|p| preset_ids.contains(&p.id)).cloned().collect()
    };
    if !preset_ids.is_empty() && selected_presets.len() != preset_ids.len() {
        return Err("部分预设未找到".to_string());
    }

    // 带上所选预设各自所属的分组
    let group_ids: Vec<String> = selected_presets
        .iter()
        .filter_map(|p| p.group_id.clone())
        .collect();
    let selected_groups: Vec<PresetGroup> = groups
        .iter()
        .filter(|g| group_ids.contains(&g.id))
        .cloned()
        .collect();

    let json = template::export_template(selected_presets, selected_groups)
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("写入文件失败: {}", e))?;
    Ok(())
}
```

- [ ] **Step 3: 加 `parse_template_file` 命令**

紧接上面加：

```rust
#[tauri::command]
pub async fn parse_template_file(path: String) -> Result<PresetTemplate, String> {
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("读取文件失败: {}", e))?;
    template::parse_template(&content).map_err(|e| e.to_string())
}
```

`PresetTemplate` 类型已在 `preset::models::*` 之外——它定义在 `template` 模块。当前文件顶部 `use crate::preset::template;` 已存在，用 `template::PresetTemplate` 引用即可。若上面代码用了裸 `PresetTemplate`，改成 `template::PresetTemplate`。返回类型写 `Result<template::PresetTemplate, String>`。

- [ ] **Step 4: lib.rs 改 invoke_handler 注册**

在 `src-tauri/src/lib.rs` 的 `invoke_handler` 宏里，找到模板那两行：

```rust
            // Template commands
            ipc_commands::export_template,
            ipc_commands::import_template,
```

替换为：

```rust
            // Preset import/export
            ipc_commands::export_presets_to_file,
            ipc_commands::parse_template_file,
```

- [ ] **Step 5: 编译验证**

Run: `cd src-tauri && cargo build`
Expected: 编译通过，无 `import_template`/`export_template` 残留引用错误。

- [ ] **Step 6: Commit（含 Task 2）**

```bash
git add src-tauri/src/preset/template.rs src-tauri/src/ipc_commands.rs src-tauri/src/lib.rs
git commit -m "feat(preset): add export_presets_to_file & parse_template_file IPC

Replace unused export_template/import_template IPC with file-based
export (writes pretty JSON) and parse-only import (returns PresetTemplate
for frontend conflict resolution). Reuses existing template::PresetTemplate."
```

---

### Task 4: 前端依赖与类型

**Files:**
- Modify: `package.json`
- Modify: `src/types/preset.ts`

**Interfaces:**
- Produces: `PresetTemplate` 前端类型；`@tauri-apps/plugin-dialog` 可用。

- [ ] **Step 1: 装 dialog JS 插件**

Run: `npm install @tauri-apps/plugin-dialog`
Expected: `package.json` 出现 `"@tauri-apps/plugin-dialog": "^2"`。

- [ ] **Step 2: 加 `PresetTemplate` 类型**

在 `src/types/preset.ts` 末尾加：

```ts
export interface PresetTemplate {
  version: string; // "1.0"
  exported_at: string;
  presets: Preset[];
  groups: PresetGroup[];
}
```

- [ ] **Step 3: 类型检查**

Run: `npm run build`
Expected: tsc 编译通过（若无其他既有错误）。如报已存在类型错误与本任务无关，忽略。

- [ ] **Step 4: Commit**

```bash
git add package.json package-lock.json src/types/preset.ts
git commit -m "feat(preset): add PresetTemplate type & dialog plugin JS dep"
```

---

### Task 5: `PresetImportDialog` 冲突解决 Modal

**Files:**
- Create: `src/components/preset/PresetImportDialog.tsx`

**Interfaces:**
- Consumes: `invoke("save_preset", { preset })`、`invoke("save_preset_group", { group })`、`invoke("load_presets")`、`invoke("load_preset_groups")`；`usePresetStore` 的 `setPresets`/`setGroups`。
- Produces: 默认导出的 React 组件，props `{ open: boolean; payload: PresetTemplate | null; onClose: () => void }`。

**逐项决策规则（实现要点）：**
- 每个 `payload.presets[i]` 对比当前 `usePresetStore.presets`：
  - **id 不存在** → 行只读，显示绿色「将新增」标签，`decision='add'`。
  - **id 已存在** → `Radio.Group`，默认 `'skip'`，选项：`跳过(skip)` / `覆盖(overwrite)` / `新增副本(copy)`。
- 损坏项（缺关键字段）→ 行灰显禁用，标「损坏」，`decision='skip'`，不阻断其他。

**分组解析（应用时执行，不弹窗）：**
- 先遍历要 apply 的预设的 `group_id`，从 `payload.groups` 取定义。
- 目标库（当前 `groups`）按**名字**匹配：同名 → 复用现有 id；否则 `save_preset_group` 创建新分组（`crypto.randomUUID()` 生成 id），记入 `groupIdMap: 原 group_id -> 解析后 id`。
- `group_id` 在 `payload.groups` 中找不到定义 → 映射为 `undefined`（归未分组）。

**应用顺序：** 先建分组拿映射 → 再按每项 decision 调 `save_preset`（`overwrite`=原 id；`copy`=新 id + `name + "(副本)"`；`add`=原 id；`skip`=跳过）→ 重新 `load_presets`/`load_preset_groups` 刷 store → message 汇总。

- [ ] **Step 1: 写组件**

创建 `src/components/preset/PresetImportDialog.tsx`：

```tsx
import React, { useMemo, useState } from "react";
import { Modal, Radio, Tag, Table, message } from "antd";
import { invoke } from "@tauri-apps/api/core";
import { usePresetStore } from "../../stores/presetStore";
import { Preset, PresetGroup, PresetTemplate } from "../../types/preset";

type Decision = "skip" | "overwrite" | "copy" | "add";

interface Props {
  open: boolean;
  payload: PresetTemplate | null;
  onClose: () => void;
}

const PresetImportDialog: React.FC<Props> = ({ open, payload, onClose }) => {
  const existingPresets = usePresetStore((s) => s.presets);
  const existingGroups = usePresetStore((s) => s.groups);
  const setPresets = usePresetStore((s) => s.setPresets);
  const setGroups = usePresetStore((s) => s.setGroups);

  // 每个导入预设的状态：是否 id 冲突、是否损坏、当前决策
  const rows = useMemo(() => {
    if (!payload) return [];
    return payload.presets.map((p) => {
      const corrupted = !p.id || !p.name || !Array.isArray(p.commands);
      const conflict = !corrupted && existingPresets.some((e) => e.id === p.id);
      const groupName =
        p.group_id ? payload.groups.find((g) => g.id === p.group_id)?.name : undefined;
      return { preset: p, conflict, corrupted, groupName, decision: conflict ? "skip" : ("add" as Decision) };
    });
  }, [payload, existingPresets]);

  const [decisions, setDecisions] = useState<Record<string, Decision>>({});

  // payload 变化时重置决策
  React.useEffect(() => {
    const init: Record<string, Decision> = {};
    rows.forEach((r) => (init[r.preset.id] = r.decision));
    setDecisions(init);
  }, [payload]); // eslint-disable-line react-hooks/exhaustive-deps

  const setDecision = (id: string, d: Decision) =>
    setDecisions((m) => ({ ...m, [id]: d }));

  const apply = async () => {
    if (!payload) return;
    let imported = 0;
    let skipped = 0;

    // 1) 分组解析：按名字匹配，缺失则创建
    const groupIdMap: Record<string, string | undefined> = {};
    const neededGroupIds = new Set(
      rows
        .filter((r) => decisions[r.preset.id] !== "skip" && r.preset.group_id)
        .map((r) => r.preset.group_id as string)
    );
    for (const gid of neededGroupIds) {
      const def = payload.groups.find((g) => g.id === gid);
      if (!def) {
        groupIdMap[gid] = undefined; // 悬空 → 未分组
        continue;
      }
      const sameName = existingGroups.find((g) => g.name === def.name);
      if (sameName) {
        groupIdMap[gid] = sameName.id;
      } else {
        const newGroup: PresetGroup = {
          ...def,
          id: crypto.randomUUID(),
        };
        await invoke("save_preset_group", { group: newGroup });
        groupIdMap[gid] = newGroup.id;
      }
    }

    // 2) 按决策 apply 预设
    for (const r of rows) {
      const d = decisions[r.preset.id];
      if (!d || d === "skip") {
        skipped++;
        continue;
      }
      let p: Preset = { ...r.preset };
      // 重映射 group_id
      if (p.group_id) p.group_id = groupIdMap[p.group_id];
      if (d === "copy") {
        p = { ...p, id: crypto.randomUUID(), name: `${p.name}(副本)` };
      }
      // add / overwrite 都用原 id（overwrite 时 save_preset 的 upsert 替换现有）
      await invoke("save_preset", { preset: p });
      imported++;
    }

    // 3) 刷新 store
    const presets = await invoke<Preset[]>("load_presets");
    const groups = await invoke<PresetGroup[]>("load_preset_groups");
    setPresets(presets);
    setGroups(groups);

    message.success(`导入 ${imported} 条，跳过 ${skipped} 条`);
    onClose();
  };

  return (
    <Modal
      title="导入预设"
      open={open}
      onOk={apply}
      onCancel={onClose}
      okText="应用导入"
      cancelText="取消"
      width={640}
      destroyOnClose
    >
      <Table
        size="small"
        pagination={false}
        rowKey={(r) => r.preset.id}
        dataSource={rows}
        columns={[
          {
            title: "名称",
            dataIndex: ["preset", "name"],
            render: (name: string, r) =>
              r.corrupted ? <span style={{ color: "#999" }}>{name || "(损坏)"} <Tag color="red">损坏</Tag></span> : name,
          },
          { title: "分组", dataIndex: "groupName", render: (n?: string) => n ?? "—" },
          {
            title: "状态",
            render: (_, r) =>
              r.corrupted ? (
                <Tag>跳过</Tag>
              ) : r.conflict ? (
                <Tag color="orange">ID 冲突</Tag>
              ) : (
                <Tag color="green">将新增</Tag>
              ),
          },
          {
            title: "处理",
            render: (_, r) => {
              if (r.corrupted) return <span style={{ color: "#999" }}>—</span>;
              if (!r.conflict) return <Tag color="green">新增</Tag>;
              return (
                <Radio.Group
                  value={decisions[r.preset.id] ?? "skip"}
                  onChange={(e) => setDecision(r.preset.id, e.target.value)}
                  size="small"
                >
                  <Radio.Button value="skip">跳过</Radio.Button>
                  <Radio.Button value="overwrite">覆盖</Radio.Button>
                  <Radio.Button value="copy">新增副本</Radio.Button>
                </Radio.Group>
              );
            },
          },
        ]}
      />
    </Modal>
  );
};

export default PresetImportDialog;
```

- [ ] **Step 2: 类型检查**

Run: `npm run build`
Expected: tsc 通过（本组件无类型错误）。

- [ ] **Step 3: Commit**

```bash
git add src/components/preset/PresetImportDialog.tsx
git commit -m "feat(preset): add PresetImportDialog with per-item conflict resolution"
```

---

### Task 6: PresetPanel 接入导入/导出按钮

**Files:**
- Modify: `src/components/preset/PresetPanel.tsx`

**Interfaces:**
- Consumes: `invoke("export_presets_to_file", { path, presetIds })`、`invoke("parse_template_file", { path })`；`save`/`open` from `@tauri-apps/plugin-dialog`；Task 5 的 `PresetImportDialog`。

- [ ] **Step 1: 加 import**

`src/components/preset/PresetPanel.tsx` 顶部加：

```tsx
import { save, open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { message } from "antd"; // 若已从 antd 解构 message 则跳过
import { ImportOutlined, ExportOutlined } from "@ant-design/icons";
import PresetImportDialog from "./PresetImportDialog";
import { PresetTemplate } from "../../types/preset";
```

检查文件顶部已有的 antd import 行：原为 `import { Button, Tree, Space, Empty, Typography, Popconfirm, Tag, Modal, Input } from "antd";`——把 `message` 加进去；图标行加入 `ImportOutlined, ExportOutlined`。

- [ ] **Step 2: 组件内加 state 与处理函数**

在 `PresetPanel` 组件内（`const [groupModal, ...]` 附近）加：

```tsx
const [importPayload, setImportPayload] = useState<PresetTemplate | null>(null);

const handleExport = async (presetIds: string[]) => {
  // presetIds 空 = 全量
  if (presetIds.length === 0 && presets.length === 0) {
    message.warning("没有可导出的预设");
    return;
  }
  const path = await save({
    filters: [{ name: "TermCraft 预设", extensions: ["tc-presets.json", "json"] }],
    defaultPath: "presets.tc-presets.json",
  });
  if (!path) return;
  try {
    await invoke("export_presets_to_file", { path, presetIds });
    message.success(`已导出到 ${path}`);
  } catch (e) {
    message.error(`导出失败: ${e}`);
  }
};

const handleImport = async () => {
  const path = await open({
    filters: [{ name: "TermCraft 预设", extensions: ["tc-presets.json", "json"] }],
    multiple: false,
  });
  if (!path) return;
  try {
    const payload = await invoke<PresetTemplate>("parse_template_file", { path });
    setImportPayload(payload);
  } catch (e) {
    message.error(`导入失败: ${e}`);
  }
};
```

- [ ] **Step 3: 工具栏加按钮**

把工具栏 `<Space>` 改成（在「新建分组」Button 之后加两个按钮）：

```tsx
      <Space style={{ width: "100%", marginBottom: 8 }}>
        <Button type="dashed" icon={<PlusOutlined />} size="small" block
          onClick={() => { setSelectedPreset(null); setEditorOpen(true); }}>
          新建预设
        </Button>
        <Button type="dashed" icon={<PlusOutlined />} size="small"
          onClick={handleCreateGroup}>
          新建分组
        </Button>
        <Button type="dashed" icon={<ImportOutlined />} size="small"
          onClick={handleImport}>
          导入
        </Button>
        <Button type="dashed" icon={<ExportOutlined />} size="small"
          onClick={() => handleExport([])}>
          导出全部
        </Button>
      </Space>
```

- [ ] **Step 4: 预设行加导出按钮**

在 `renderPresetTitle` 里，delete 按钮旁加导出按钮。原 delete 行：

```tsx
        <Popconfirm title="确认删除此预设?" onConfirm={() => handleDeletePreset(p.id)}>
          <Button type="text" icon={<DeleteOutlined />} size="small" danger style={{ opacity: 0.5, flexShrink: 0 }} />
        </Popconfirm>
```

在其后加：

```tsx
        <Button type="text" icon={<ExportOutlined />} size="small" style={{ opacity: 0.5, flexShrink: 0 }}
          onClick={() => handleExport([p.id])} />
```

- [ ] **Step 5: 挂载导入 Modal**

在组件 return 的末尾（group `<Modal>` 之后）加：

```tsx
      <PresetImportDialog
        open={!!importPayload}
        payload={importPayload}
        onClose={() => setImportPayload(null)}
      />
```

- [ ] **Step 6: 类型检查**

Run: `npm run build`
Expected: tsc 通过。

- [ ] **Step 7: Commit**

```bash
git add src/components/preset/PresetPanel.tsx
git commit -m "feat(preset): wire import/export buttons into PresetPanel"
```

---

### Task 7: 集成手动验证

**Files:** 无（运行验证）

- [ ] **Step 1: 启动 dev**

Run: `npm run tauri dev`
Expected: 应用启动，无编译/运行错误。

- [ ] **Step 2: 全量导出**

预设面板有至少 1 个预设时点「导出全部」→ 选保存路径 → 文件生成。打开文件确认 `version:"1.0"`、含全部预设与分组。

- [ ] **Step 3: 单预设导出**

某预设行的导出图标 → 保存 → 文件只含该预设 + 其所属分组。

- [ ] **Step 4: 空库全量导出拦截**

删光预设后点「导出全部」→ 提示「没有可导出的预设」，不弹保存框。

- [ ] **Step 5: 导入到空库**

清空本地预设（或新机器）→ 「导入」选刚才的全量文件 → 全部新增，分组按名创建。

- [ ] **Step 6: 同名分组不重复创建**

目标已有同名分组 → 导入 → 分组映射到现有 id，不新建重复分组。

- [ ] **Step 7: ID 冲突三选项**

导入文件含本地已存在 id 的预设 → Modal 显示「ID 冲突」行 → 分别测 跳过/覆盖/新增副本 → 确认各自行为正确。

- [ ] **Step 8: 损坏/错误版本文件**

手动改导出文件把 `version` 改成 `"9"` 或破坏 JSON → 导入 → 报错，不进 Modal。

- [ ] **Step 9: group_id 悬空**

导出文件里某预设 `group_id` 指向不在 `groups` 里的 id → 导入 → 该预设归未分组。

- [ ] **Step 10: 枚举字段损坏**

把某预设 `execution_mode` 改成无效值 → 导入 → 该行显示「损坏」灰显跳过，其他预设正常导入。

- [ ] **Step 11: 旧命令已移除**

确认应用正常、编译无 `export_template`/`import_template` 残留（Task 3 已保证）。

---

## Self-Review 记录

- **Spec 覆盖**：spec 每节都有对应 Task——格式(Task 2/3)、后端命令(Task 2/3)、前端类型(Task 4)、入口与流程(Task 6)、冲突 Modal(Task 5)、错误边界(Task 7 验证)、文件清单(各 Task)。
- **占位符**：无 TBD/TODO；每步含实际代码或确切命令。
- **类型一致性**：`parse_template`/`parse_template_file` 返回 `PresetTemplate`；前端 `PresetTemplate` 字段与后端 `template::PresetTemplate` 一致（version String/exported_at String/presets/groups）；`export_presets_to_file` 参数 `preset_ids: Vec<String>` ↔ 前端 `presetIds: string[]`（Tauri 自动 camelCase）。
- **与 CLAUDE.md 一致**：无测试基建，用编译+手动验证替代 TDD。
