import React, { useState, useEffect } from "react";
import { Button, Tree, Space, Empty, Typography, Popconfirm, Tag, Modal, Input } from "antd";
import { PlusOutlined, PlayCircleOutlined, DeleteOutlined, EditOutlined, ExportOutlined } from "@ant-design/icons";
import PresetEditor from "./PresetEditor";
import PresetRunner from "./PresetRunner";
import { usePresetStore } from "../../stores/presetStore";
import { useAppStore } from "../../stores/appStore";
import { Preset, PresetGroup } from "../../types/preset";

const PresetPanel: React.FC = () => {
  const presets = usePresetStore((s) => s.presets);
  const groups = usePresetStore((s) => s.groups);
  const [editorOpen, setEditorOpen] = useState(false);
  const [selectedPreset, setSelectedPreset] = useState<Preset | null>(null);
  const [runnerOpen, setRunnerOpen] = useState(false);
  const [runningPreset, setRunningPreset] = useState<Preset | null>(null);
  // Group create/rename modal. `renameGroup` is the group being renamed, or
  // null for creation. Using an in-app Modal avoids the browser prompt, whose
  // title bar can't be styled (it shows the page origin like localhost:1420).
  const [groupModal, setGroupModal] = useState<{ rename: PresetGroup | null; value: string } | null>(null);
  // Expanded tree keys. New groups auto-expand; manual collapse/expand is preserved.
  const [expandedKeys, setExpandedKeys] = useState<React.Key[]>([]);
  useEffect(() => {
    setExpandedKeys((prev) => {
      const have = new Set(prev);
      const next = [...prev];
      for (const g of groups) {
        const k = `group-${g.id}`;
        if (!have.has(k)) next.push(k);
      }
      if (!have.has("ungrouped")) next.push("ungrouped");
      return next;
    });
  }, [groups]);

  // Render a preset node title: run + name + mode tag + edit/delete actions.
  // Uses a flex layout so the name ellipsizes in the middle while the tag and
  // action buttons stay pinned to the right and aligned across rows regardless
  // of name length.
  const renderPresetTitle = (p: Preset) => (
    <div style={{ display: "flex", alignItems: "center", gap: 4, width: "100%" }}>
      <PlayCircleOutlined style={{ color: "var(--accent-color)", cursor: "pointer", flexShrink: 0 }}
        onClick={() => handleRunPreset(p)} />
      <Typography.Text ellipsis style={{ flex: 1, minWidth: 0 }}>{p.name}</Typography.Text>
      <Tag color={p.execution_mode.type === "Single" ? "blue" : p.execution_mode.type === "Batch" ? "green" : "orange"} style={{ fontSize: 10, flexShrink: 0, marginRight: 0 }}>
        {p.execution_mode.type}
      </Tag>
      <Button type="text" icon={<EditOutlined />} size="small" style={{ opacity: 0.5, flexShrink: 0 }}
        onClick={() => handleEditPreset(p)} />
      <Popconfirm title="确认删除此预设?" onConfirm={() => handleDeletePreset(p.id)}>
        <Button type="text" icon={<DeleteOutlined />} size="small" danger style={{ opacity: 0.5, flexShrink: 0 }} />
      </Popconfirm>
    </div>
  );

  // Build tree data from groups and presets
  const treeData = [
    ...groups.map((group) => ({
      key: `group-${group.id}`,
      title: (
        <Space>
          <Typography.Text>{group.name}</Typography.Text>
          <Button type="text" icon={<EditOutlined />} size="small" style={{ opacity: 0.5 }}
            onClick={() => handleRenameGroup(group)} />
          <Button type="text" icon={<DeleteOutlined />} size="small" danger style={{ opacity: 0.5 }}
            onClick={() => handleDeleteGroup(group.id)} />
        </Space>
      ),
      children: presets
        .filter((p) => p.group_id === group.id)
        .map((p) => ({ key: `preset-${p.id}`, title: renderPresetTitle(p) })),
    })),
    // Presets without a group — only render this node when there actually are
    // any, so we don't show an empty "未分组" entry.
    ...(presets.some((p) => !p.group_id)
      ? [{
          key: "ungrouped",
          title: <Typography.Text style={{ color: "var(--text-secondary)" }}>未分组</Typography.Text>,
          children: presets
            .filter((p) => !p.group_id)
            .map((p) => ({ key: `preset-${p.id}`, title: renderPresetTitle(p) })),
        }]
      : []),
  ];

  const handleRunPreset = (preset: Preset) => {
    setRunningPreset(preset);
    setRunnerOpen(true);
  };

  const handleEditPreset = (preset: Preset) => {
    setSelectedPreset(preset);
    setEditorOpen(true);
  };

  const handleCreateGroup = () => {
    setGroupModal({ rename: null, value: "" });
  };

  const handleRenameGroup = (group: PresetGroup) => {
    setGroupModal({ rename: group, value: group.name });
  };

  const submitGroupModal = async () => {
    if (!groupModal) return;
    const name = groupModal.value.trim();
    if (!name) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      if (groupModal.rename) {
        // Rename existing group (same id, new name). Backend upserts.
        const updated: PresetGroup = { ...groupModal.rename, name };
        await invoke("save_preset_group", { group: updated });
        usePresetStore.getState().updateGroup(updated);
      } else {
        const group: PresetGroup = { id: crypto.randomUUID(), name };
        await invoke("save_preset_group", { group });
        usePresetStore.getState().addGroup(group);
      }
    } catch (e) {
      console.error("Save group failed:", e);
    }
    setGroupModal(null);
  };

  const handleDeleteGroup = async (id: string) => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("delete_preset_group", { id });
      usePresetStore.getState().removeGroup(id);
    } catch (e) {
      console.error("Delete group failed:", e);
    }
  };

  const handleDeletePreset = async (id: string) => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("delete_preset", { id });
      usePresetStore.getState().removePreset(id);
    } catch (e) {
      console.error("Delete preset failed:", e);
    }
  };

  return (
    <div>
      <Space style={{ width: "100%", marginBottom: 8 }}>
        <Button type="dashed" icon={<PlusOutlined />} size="small" block
          onClick={() => { setSelectedPreset(null); setEditorOpen(true); }}>
          新建预设
        </Button>
        <Button type="dashed" icon={<PlusOutlined />} size="small"
          onClick={handleCreateGroup}>
          新建分组
        </Button>
      </Space>

      {presets.length === 0 && groups.length === 0 ? (
        <Empty description="暂无预设" image={Empty.PRESENTED_IMAGE_SIMPLE} />
      ) : (
        <Tree
          className="preset-tree"
          treeData={treeData}
          expandedKeys={expandedKeys}
          onExpand={setExpandedKeys}
        />
      )}

      {editorOpen && (
        <PresetEditor
          preset={selectedPreset}
          onClose={() => setEditorOpen(false)}
        />
      )}

      {runnerOpen && runningPreset && (
        <PresetRunner
          preset={runningPreset}
          onClose={() => setRunnerOpen(false)}
        />
      )}

      <Modal
        title={groupModal?.rename ? "重命名分组" : "新建分组"}
        open={!!groupModal}
        onOk={submitGroupModal}
        onCancel={() => setGroupModal(null)}
        okText="保存"
        cancelText="取消"
        destroyOnClose
        width={360}
      >
        <Input
          autoFocus
          placeholder="分组名称"
          value={groupModal?.value ?? ""}
          onChange={(e) => setGroupModal((m) => (m ? { ...m, value: e.target.value } : m))}
          onPressEnter={submitGroupModal}
        />
      </Modal>
    </div>
  );
};

export default PresetPanel;