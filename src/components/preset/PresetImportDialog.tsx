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
      return { preset: p, conflict, corrupted, groupName, decision: (corrupted || conflict ? "skip" : "add") as Decision };
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
