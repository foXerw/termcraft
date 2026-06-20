import React, { useMemo, useState } from "react";
import { Modal, Radio, Tag, Table, message } from "antd";
import { invoke } from "@tauri-apps/api/core";
import { usePresetStore } from "../../stores/presetStore";
import { Preset, PresetGroup, ParsedTemplate } from "../../types/preset";

type Decision = "skip" | "overwrite" | "copy" | "add";

interface Props {
  open: boolean;
  payload: ParsedTemplate | null;
  onClose: () => void;
}

// A row in the import table. Normal preset rows carry conflict/decision state;
// corrupted rows (presets that failed per-item parse) are always skipped and
// rendered greyed-out.
type Row =
  | { kind: "preset"; preset: Preset; conflict: boolean; corrupted: boolean; groupName?: string; decision: Decision }
  | { kind: "corrupt"; index: number; name: string; error: string };

const PresetImportDialog: React.FC<Props> = ({ open, payload, onClose }) => {
  const existingPresets = usePresetStore((s) => s.presets);
  const existingGroups = usePresetStore((s) => s.groups);
  const setPresets = usePresetStore((s) => s.setPresets);
  const setGroups = usePresetStore((s) => s.setGroups);

  // 每个导入预设的状态：是否 id 冲突；损坏的预设（来自后端逐条解析失败）单独成行
  const rows = useMemo<Row[]>(() => {
    if (!payload) return [];
    const presetRows: Row[] = payload.presets.map((p) => {
      const conflict = existingPresets.some((e) => e.id === p.id);
      const groupName =
        p.group_id ? payload.groups.find((g) => g.id === p.group_id)?.name : undefined;
      return {
        kind: "preset" as const,
        preset: p,
        conflict,
        corrupted: false,
        groupName,
        decision: (conflict ? "skip" : "add") as Decision,
      };
    });
    const corruptRows: Row[] = payload.corrupted.map((c) => ({
      kind: "corrupt" as const,
      index: c.index,
      name: c.name ?? "(损坏)",
      error: c.error,
    }));
    // 保持原始顺序：按 corrupted.index 与 preset 在 presets 数组中的位置交错。
    // preset 行没有 index，但 presets 数组原本就是按顺序的，所以先标记再合并排序。
    const indexed: Row[] = presetRows.map((r, i) => r);
    // 简单起见：preset 行顺序在前，corrupt 行按 index 排序追加在后。
    // （per-item 失败只影响展示，不影响应用流程。）
    return [...indexed, ...corruptRows];
  }, [payload, existingPresets]);

  const [decisions, setDecisions] = useState<Record<string, Decision>>({});

  // payload 变化时重置决策（仅 preset 行有 decision）
  React.useEffect(() => {
    const init: Record<string, Decision> = {};
    rows.forEach((r) => {
      if (r.kind === "preset") init[r.preset.id] = r.decision;
    });
    setDecisions(init);
  }, [payload]); // eslint-disable-line react-hooks/exhaustive-deps

  const setDecision = (id: string, d: Decision) =>
    setDecisions((m) => ({ ...m, [id]: d }));

  const apply = async () => {
    if (!payload) return;
    let imported = 0;
    let skipped = 0;
    let failed = 0;

    try {
      // 1) 分组解析：按名字匹配，缺失则创建（分组创建失败计入 failed，不中断）
      const groupIdMap: Record<string, string | undefined> = {};
      const neededGroupIds = new Set(
        rows
          .filter((r): r is Extract<Row, { kind: "preset" }> =>
            r.kind === "preset" && decisions[r.preset.id] !== "skip" && !!r.preset.group_id)
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
          const newGroup: PresetGroup = { ...def, id: crypto.randomUUID() };
          try {
            await invoke("save_preset_group", { group: newGroup });
            groupIdMap[gid] = newGroup.id;
          } catch (e) {
            failed++;
            console.error("保存分组失败:", e);
          }
        }
      }

      // 2) 按决策 apply 预设（损坏行永远跳过；单项失败不中断其余）
      for (const r of rows) {
        if (r.kind !== "preset") {
          skipped++;
          continue;
        }
        const d = decisions[r.preset.id];
        if (!d || d === "skip") {
          skipped++;
          continue;
        }
        let p: Preset = { ...r.preset };
        if (p.group_id) p.group_id = groupIdMap[p.group_id];
        if (d === "copy") {
          p = { ...p, id: crypto.randomUUID(), name: `${p.name}(副本)` };
        }
        try {
          await invoke("save_preset", { preset: p });
          imported++;
        } catch (e) {
          failed++;
          console.error("保存预设失败:", e);
        }
      }

      // 成功路径：报告导入/跳过/失败计数
      const parts = [`导入 ${imported} 条`, `跳过 ${skipped} 条`];
      if (failed > 0) parts.push(`失败 ${failed} 条`);
      message.success(parts.join("，"));
    } catch (e) {
      // 外层异常（理论上每项已有 try/catch，这里防御性兜底）
      message.error(`导入过程出错: ${e}`);
    } finally {
      // 始终刷新 store + 关闭弹窗，避免半成功后状态不一致；
      // 刷新本身用 .catch 兜底，确保 finally 不会再次抛出
      const presets = await invoke<Preset[]>("load_presets").catch(() => [] as Preset[]);
      const groups = await invoke<PresetGroup[]>("load_preset_groups").catch(() => [] as PresetGroup[]);
      setPresets(presets);
      setGroups(groups);
      onClose();
    }
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
      <Table<Row>
        size="small"
        pagination={false}
        rowKey={(r) => (r.kind === "preset" ? r.preset.id : `corrupt-${r.index}`)}
        dataSource={rows}
        columns={[
          {
            title: "名称",
            render: (_, r) => {
              if (r.kind === "corrupt") {
                return (
                  <span style={{ color: "#999" }}>
                    {r.name} <Tag color="red">损坏</Tag>
                  </span>
                );
              }
              return r.preset.name;
            },
          },
          {
            title: "分组",
            render: (_, r) => (r.kind === "preset" ? r.groupName ?? "—" : "—"),
          },
          {
            title: "状态",
            render: (_, r) => {
              if (r.kind === "corrupt") return <Tag>跳过</Tag>;
              if (r.conflict) return <Tag color="orange">ID 冲突</Tag>;
              return <Tag color="green">将新增</Tag>;
            },
          },
          {
            title: "处理",
            render: (_, r) => {
              if (r.kind === "corrupt") return <span style={{ color: "#999" }}>—</span>;
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
