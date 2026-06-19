import React, { useEffect } from "react";
import { Typography, List, Button, Space } from "antd";
import { PlusOutlined, LinkOutlined, ThunderboltOutlined } from "@ant-design/icons";
import ConnectionCard from "../connection/ConnectionCard";
import PresetPanel from "../preset/PresetPanel";
import { useConnectionStore } from "../../stores/connectionStore";
import { usePresetStore } from "../../stores/presetStore";
import { useAppStore } from "../../stores/appStore";

interface SidebarProps {
  collapsed: boolean;
}

const Sidebar: React.FC<SidebarProps> = ({ collapsed }) => {
  const configs = useConnectionStore((s) => s.configs);
  const setConfigs = useConnectionStore((s) => s.setConfigs);
  const setReachStatus = useConnectionStore((s) => s.setReachStatus);
  const presets = usePresetStore((s) => s.presets);
  const setPresets = usePresetStore((s) => s.setPresets);
  const groups = usePresetStore((s) => s.groups);
  const setGroups = usePresetStore((s) => s.setGroups);
  const openConnectionForm = useAppStore((s) => s.openConnectionForm);

  // Load data on mount
  useEffect(() => {
    async function loadData() {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const configsData = await invoke("load_connection_configs");
        setConfigs(configsData as any[]);
        const presetsData = await invoke("load_presets");
        setPresets(presetsData as any[]);
        const groupsData = await invoke("load_preset_groups");
        setGroups(groupsData as any[]);
      } catch (e) {
        console.error("Failed to load data:", e);
      }
    }
    loadData();
  }, []);

  // Subscribe to reachability status pushes from the backend.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen<{ id: string; status: string; latency_ms?: number; last_checked?: string }>(
        "connection_status",
        (e) => {
          setReachStatus(e.payload.id, {
            status: e.payload.status as any,
            latencyMs: e.payload.latency_ms,
            lastChecked: e.payload.last_checked,
          });
        }
      );
    })();
    return () => { unlisten?.(); };
  }, [setReachStatus]);

  // Whenever the connection list changes, re-register probe targets.
  // Only connections with a host (SSH/Telnet) are probed.
  useEffect(() => {
    (async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const targets = configs
          .filter((c) => !!c.host)
          .map((c) => [
            c.id,
            c.host,
            c.port ?? (c.conn_type === "Telnet" ? 23 : 22),
          ]);
        await invoke("set_reachability_targets", { targets });
      } catch (e) {
        console.error("Failed to sync reachability targets:", e);
      }
    })();
  }, [configs]);

  if (collapsed) {
    return <div className="sidebar collapsed" />;
  }

  return (
    <div className="sidebar">
      <div className="sidebar-section">
        <div className="sidebar-section-title">
          <Space>
            <LinkOutlined />
            连接列表
          </Space>
        </div>
        <Button type="dashed" icon={<PlusOutlined />} block size="small" style={{ marginBottom: 8 }}
          onClick={() => openConnectionForm()}>
          新建连接
        </Button>
        <List
          size="small"
          dataSource={configs}
          renderItem={(config) => (
            <List.Item style={{ padding: "4px 0", border: "none" }}>
              <ConnectionCard config={config} />
            </List.Item>
          )}
          locale={{ emptyText: "暂无保存的连接" }}
        />
      </div>

      <div className="sidebar-section" style={{ borderTop: "1px solid var(--border-color)" }}>
        <div className="sidebar-section-title">
          <Space>
            <ThunderboltOutlined />
            预设命令
          </Space>
        </div>
        <PresetPanel />
      </div>
    </div>
  );
};

export default Sidebar;