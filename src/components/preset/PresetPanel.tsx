import React, { useState } from "react";
import { Button, Tree, Space, Empty, Typography, Popconfirm, Tag } from "antd";
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

  // Build tree data from groups and presets
  const treeData = [
    ...groups.map((group) => ({
      key: `group-${group.id}`,
      title: (
        <Space>
          <Typography.Text>{group.name}</Typography.Text>
          <Button type="text" icon={<DeleteOutlined />} size="small" style={{ opacity: 0.5 }}
            onClick={() => handleDeleteGroup(group.id)} />
        </Space>
      ),
      children: presets
        .filter((p) => p.group_id === group.id)
        .map((p) => ({
          key: `preset-${p.id}`,
          title: (
            <Space>
              <PlayCircleOutlined style={{ color: "var(--accent-color)", cursor: "pointer" }}
                onClick={() => handleRunPreset(p)} />
              <Typography.Text ellipsis style={{ maxWidth: 140 }}>{p.name}</Typography.Text>
              <Tag color={p.execution_mode.type === "Single" ? "blue" : p.execution_mode.type === "Batch" ? "green" : "orange"} style={{ fontSize: 10 }}>
                {p.execution_mode.type}
              </Tag>
            </Space>
          ),
        })),
    })),
    // Presets without a group
    {
      key: "ungrouped",
      title: <Typography.Text style={{ color: "var(--text-secondary)" }}>未分组</Typography.Text>,
      children: presets
        .filter((p) => !p.group_id)
        .map((p) => ({
          key: `preset-${p.id}`,
          title: (
            <Space>
              <PlayCircleOutlined style={{ color: "var(--accent-color)", cursor: "pointer" }}
                onClick={() => handleRunPreset(p)} />
              <Typography.Text ellipsis style={{ maxWidth: 140 }}>{p.name}</Typography.Text>
            </Space>
          ),
        })),
    },
  ];

  const handleRunPreset = (preset: Preset) => {
    setRunningPreset(preset);
    setRunnerOpen(true);
  };

  const handleEditPreset = (preset: Preset) => {
    setSelectedPreset(preset);
    setEditorOpen(true);
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
      <Button type="dashed" icon={<PlusOutlined />} block size="small" style={{ marginBottom: 8 }}
        onClick={() => { setSelectedPreset(null); setEditorOpen(true); }}>
        新建预设
      </Button>

      {presets.length === 0 && groups.length === 0 ? (
        <Empty description="暂无预设" image={Empty.PRESENTED_IMAGE_SIMPLE} />
      ) : (
        <Tree treeData={treeData} defaultExpandAll />
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
    </div>
  );
};

export default PresetPanel;