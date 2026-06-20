# 预设命令导入导出设计

日期：2026-06-20
状态：已确认，待实现

## 背景

预设命令（presets）以 JSON 文件保存在用户数据目录 `dirs::data_dir()/termcraft/presets.json`（Windows: `C:\Users\<user>\AppData\Roaming\termcraft\presets.json`），分组存于同目录 `groups.json`，预设通过 `group_id` 关联分组。原子写入（先写 `.tmp` 再 rename）由 `src-tauri/src/config/store.rs` 负责。

用户希望支持预设的导入与导出，便于备份、迁移到其他机器、分享。

## 目标

- 支持导出单个预设、导出全部预设。
- 支持从文件导入预设。
- 导入时按分组名自动归组，ID 冲突逐项解决。

## 非目标

- 不做加密、不涉及密钥（预设本身不含密钥；密钥在连接配置的 keyring 中，与本功能无关）。
- 不做预设的云端同步。
- 不做跨版本 schema 迁移工具（仅留 `version` 字段为未来预留）。

## 文件交互机制

新增 `tauri-plugin-dialog` 依赖（Rust + JS），capabilities 配置 `dialog:default` 允许原生 open/save 对话框。文件 IO 全部留在 Rust，与现有 `store.rs` 模式一致；不给前端泛 fs 权限。

## 文件格式

```json
{
  "version": "1.0",
  "exported_at": "2026-06-20T12:00:00Z",
  "presets": [ /* 完整 Preset 对象 */ ],
  "groups": [ /* PresetGroup[] */ ]
}
```

- 复用现有 `template::PresetTemplate` 类型，**不新增类型**。
- `version` 固定字符串 `"1.0"`，导入时校验 `== "1.0"`，不符返回中文错误。
- `exported_at` ISO8601（`chrono::Utc::now().to_rfc3339()`），仅记录用，导入时忽略。
- `presets`：完整 `Preset[]`。
  - 单预设导出：长度为 1。
  - 全量导出：含全部预设。
- `groups`：`PresetGroup[]`。
  - 全量导出：含全部分组。
  - 单预设导出：只含该预设的所属分组；该预设无分组则为空数组。
- 文件后缀 `.tc-presets.json`，对话框过滤器默认匹配 `*.tc-presets.json` 与 `*.json`。
- 序列化用 `serde_json::to_string_pretty`（人类可读，便于版本管理/分享）。

> **与现有代码的关系（2026-06-20 修订）**：后端 `src-tauri/src/preset/template.rs` 已存在 `PresetTemplate` 类型与 `export_template`/`import_template` 逻辑，但前端从未接线（`TemplateManager.tsx` 为空占位）。本设计**复用**该类型与导出组装逻辑，不新建并行类型/命令。冲突模型从现有单一 `overwrite: bool` 升级为前端逐项决策，因此把导入拆成「解析（只读返回）」+「前端按决策 apply」两步。旧的无用命令 `export_template`/`import_template` IPC 及 `template::import_template` 函数随之移除。

## 后端（Rust）

### 新增依赖与权限

- `src-tauri/Cargo.toml` 增加 `tauri-plugin-dialog = "2"`。
- `src-tauri/src/lib.rs` 的 `tauri::Builder` 注册 `tauri_plugin_dialog::init()`。
- capabilities（`src-tauri/capabilities/default.json`）增加 `dialog:default` 权限（允许 `open` / `save` 对话框）。不授予 `fs` 权限——文件读写走自定义命令。

### 复用 `template` 模块

`src-tauri/src/preset/template.rs` 已有 `PresetTemplate` 类型与 `export_template(presets, groups) -> Result<String>`（组装 + pretty 序列化）。复用它，并新增一个只解析不 apply 的函数：

```rust
/// 解析模板字符串并校验版本，不写任何文件、不 apply。
pub fn parse_template(json: &str) -> Result<PresetTemplate, AppError> {
    let template: PresetTemplate = serde_json::from_str(json)
        .map_err(|e| AppError::Preset(format!("无法解析预设文件: {}", e)))?;
    if template.version != "1.0" {
        return Err(AppError::Preset(format!("不支持的预设文件版本: {}", template.version)));
    }
    Ok(template)
}
```

移除现有 `template::import_template`（单一 overwrite 语义被前端逐项决策取代）。

### 新增 IPC 命令（`src-tauri/src/ipc_commands.rs`）

**`export_presets_to_file(path: String, preset_ids: Vec<String>) -> Result<(), String>`**

- `preset_ids` 为空时视为「全量」：导出全部预设 + 全部分组。
- 非空时：取这些 id 的预设 + 各自所属分组。
- `store::load_presets()` + `store::load_preset_groups()` 取数据，按 id 过滤，调 `template::export_template(selected_presets, selected_groups)` 拿 JSON 字符串。
- 用 `fs::write`（或 `store` 内 `atomic_write` 的等价公开函数）写入 `path`。
- 指定 id 找不到预设 → 返回中文错误。

**`parse_template_file(path: String) -> Result<PresetTemplate, String>`**

- 读文件 → `template::parse_template(content)`（解析 + 版本校验）。
- **只读不改**：返回 `PresetTemplate` 给前端，前端弹冲突 Modal 后用现有 `save_preset` / `save_preset_group` apply。

### 移除的旧命令

- `ipc_commands::export_template`（IPC，未被前端使用）。
- `ipc_commands::import_template`（IPC，被 `parse_template_file` + 前端 apply 取代）。
- `template::import_template`（函数）。
- `lib.rs` 的 `invoke_handler` 注册中删除 `export_template` / `import_template` 两行。

### 不新增

- 不新建 `ExportFile` 类型（复用 `PresetTemplate`）。
- 不新增 apply 命令（复用现有 `save_preset` upsert by id + `save_preset_group`）。

## 前端

### 类型（`src/types/preset.ts`）

```ts
export interface PresetTemplate {
  version: string; // "1.0"
  exported_at: string;
  presets: Preset[];
  groups: PresetGroup[];
}
```

### 入口（`src/components/preset/PresetPanel.tsx`）

工具栏「新建预设 / 新建分组」旁增加：
- `导入` 按钮（`ImportOutlined`）。
- `导出全部` 按钮（`ExportOutlined`）。

每个预设行操作区（`renderPresetTitle`，run/edit/delete 旁）增加：
- `导出` 图标按钮（`ExportOutlined`，opacity 0.5 风格与现有 edit/delete 一致）。

### 导出流程

1. `save({ filters: [{ name: 'TermCraft 预设', extensions: ['tc-presets.json','json'] }], defaultPath: 'presets.tc-presets.json' })` 获取路径。用户取消则中止。
2. 全量：`invoke("export_presets_to_file", { path, presetIds: [] })`（空数组=全部）。
   单个：`invoke("export_presets_to_file", { path, presetIds: [p.id] })`。
3. 成功 `message.success` 提示导出路径。

全量导出前拦截：若 `presets.length === 0`，`message.warning("没有可导出的预设")` 并中止，不生成空文件。

### 导入流程

1. `open({ filters: [{ name: 'TermCraft 预设', extensions: ['tc-presets.json','json'] }], multiple: false })` 获取路径。取消则中止。
2. `invoke("parse_template_file", { path })` → `PresetTemplate`。失败 `message.error` 提示中文错误，中止。
3. 打开**冲突解决 Modal**。

### 冲突解决 Modal

新建组件 `src/components/preset/PresetImportDialog.tsx`，props：`open`, `payload: PresetTemplate | null`, `onClose`。

**分组解析（自动，不弹窗）**：
- 对每个导入预设的 `group_id`，从 `payload.groups` 取分组定义。
- 目标库按**分组名**匹配：
  - 同名已存在 → 用现有分组 id。
  - 不存在 → 调 `save_preset_group` 创建（新 id）。
- 预设的 `group_id` 重映射到解析后的 id。
- 导入预设 `group_id` 在 `payload.groups` 中找不到对应定义 → 该预设 `group_id` 置空（归入未分组）。

**预设冲突逐项**：
- 每个导入预设对当前库判断：
  - **id 不存在** → 显示绿色「将新增」标识，无需选择，应用时直接 `save_preset`（保留原 id）。
  - **id 已存在** → `Radio.Group`，默认「跳过」，可选：
    - `跳过`：不导入。
    - `覆盖`：`save_preset` 用原 id（替换现有）。橙色标识提醒。
    - `新增副本`：生成新 id，`name` 后缀 `(副本)`，`save_preset`。

**应用**：
- 先完成分组解析（创建缺失分组，拿到 id 映射）。
- 按每行选择调 `save_preset`。
- 完成后统一重新 `load_presets` + `load_preset_groups` 刷新 store（不逐个 add）。
- `message.success` 汇总「导入 N 条，跳过 M 条」。

## 错误处理与边界

- **校验失败**（格式/版本不符、JSON 损坏）：Rust 返回中文错误，前端 `message.error`，不进入 Modal。
- **字段容错**：导入预设缺 `commands`/`variables` → 当空数组；`execution_mode`/`wait_for` 枚举反序列化失败的预设 → Modal 中灰显、禁用选择、标「损坏」、默认跳过，不阻断其他预设。
- **分组引用悬空**：`group_id` 在导入文件 `groups` 中无对应 → `group_id` 置空，不报错。
- **全量导出空库**：拦截，`message.warning("没有可导出的预设")`，不生成文件。
- **覆盖保护**：覆盖行橙色标识，Modal 底部确认按钮「应用导入」；不加二次确认弹窗（用户已逐项选择）。
- **大文件**：预设为小型 JSON，不设大小限制。

## 涉及文件

| 文件 | 改动 |
|------|------|
| `src-tauri/Cargo.toml` | 加 `tauri-plugin-dialog = "2"` |
| `src-tauri/src/lib.rs` | 注册 `tauri_plugin_dialog::init()`；从 `invoke_handler` 移除 `export_template`/`import_template` |
| `src-tauri/capabilities/default.json` | 加 `dialog:default` |
| `src-tauri/src/preset/template.rs` | 新增 `parse_template`；移除 `import_template`（复用 `PresetTemplate`、`export_template`） |
| `src-tauri/src/ipc_commands.rs` | 新增 `export_presets_to_file`、`parse_template_file`；移除 `export_template`/`import_template` |
| `src/types/preset.ts` | `PresetTemplate` 类型 |
| `src/components/preset/PresetPanel.tsx` | 工具栏导入/导出按钮 + 行内导出按钮 |
| `src/components/preset/PresetImportDialog.tsx` | 新建冲突解决 Modal |
| `package.json` | 加 `@tauri-apps/plugin-dialog` |

## 测试要点

手动验证（项目无测试基建）：
1. 全量导出 → 文件内容含全部预设+分组，`version:"1.0"` 字段正确。
2. 单预设导出 → 文件只含该预设+其所属分组。
3. 全量导出空库 → 拦截提示，无文件生成。
4. 导入到空库 → 全部新增，分组按名创建。
5. 导入到有同名分组的目标 → 分组映射到现有 id，不重复创建。
6. 导入 ID 冲突 → 三选项分别生效（跳过/覆盖/新增副本）。
7. 导入损坏/错误版本文件 → 报错不进 Modal。
8. 导入预设 group_id 悬空 → 归入未分组。
9. 导入预设枚举字段损坏 → 该行灰显跳过，其他正常导入。
10. 旧 `export_template`/`import_template` IPC 已移除且编译通过（无残留引用）。
