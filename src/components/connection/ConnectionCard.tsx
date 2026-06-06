import React from "react";
import { Typography, Tag, Button, Space, Popconfirm } from "antd";
import { LinkOutlined, DeleteOutlined, EditOutlined } from "@ant-design/icons";
import { ConnectionConfig } from "../../types/connection";
import { useAppStore } from "../../stores/appStore";
import { useConnectionStore } from "../../stores/connectionStore";

interface ConnectionCardProps {
  config: ConnectionConfig;
}

const ConnectionCard: React.FC<ConnectionCardProps> = ({ config }) => {
  const addTab = useAppStore((s) => s.addTab);
  const setChannel = useAppStore((s) => s.setChannel);
  const removeConfig = useConnectionStore((s) => s.removeConfig);

  // 双击连接 — 自动重连
  const handleDoubleClick = async () => {
    try {
      const { invoke, Channel } = await import("@tauri-apps/api/core");
      const id = crypto.randomUUID();
      const channel = new Channel();
      setChannel(id, channel);

      if (config.conn_type === "SSH") {
        if (!config.host || !config.username || !config.auth) {
          console.error("SSH config missing required fields (host/username/auth)");
          return;
        }
        await invoke("connect_ssh", {
          id,
          host: config.host,
          port: config.port || 22,
          username: config.username,
          auth: config.auth,
          channel,
        });
      } else if (config.conn_type === "Telnet") {
        if (!config.host) {
          console.error("Telnet config missing required field (host)");
          return;
        }
        await invoke("connect_telnet", {
          id,
          host: config.host,
          port: config.port || 23,
          channel,
        });
      } else if (config.conn_type === "LocalShell") {
        await invoke("connect_local", {
          id,
          shell: config.shell || null,
          channel,
        });
      }

      addTab({
        id,
        connectionId: id,
        title: config.name,
        connType: config.conn_type,
        alive: true,
      });
    } catch (e) {
      console.error("Double-click connect failed:", e);
    }
  };

  // 删除连接配置
  const handleDelete = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("delete_connection_config", { id: config.id });
      removeConfig(config.id);
    } catch (e) {
      console.error("Delete failed:", e);
    }
  };

  // 编辑（暂时只打印，后续实现编辑弹窗）
  const handleEdit = (e: React.MouseEvent) => {
    e.stopPropagation();
    console.log("Edit config:", config.id);
  };

  const typeColors: Record<string, string> = {
    SSH: "blue",
    Telnet: "green",
    LocalShell: "orange",
  };

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        width: "100%",
        padding: "4px 8px",
        borderRadius: 4,
        cursor: "pointer",
      }}
      onDoubleClick={handleDoubleClick}
    >
      <Space size={4}>
        <LinkOutlined style={{ color: typeColors[config.conn_type] }} />
        <Typography.Text ellipsis style={{ maxWidth: 140 }}>{config.name}</Typography.Text>
        <Tag color={typeColors[config.conn_type]} style={{ fontSize: 10 }}>{config.conn_type}</Tag>
      </Space>
      <Space size={4} onDoubleClick={(e) => e.stopPropagation()}>
        <Button type="text" icon={<EditOutlined />} size="small" style={{ opacity: 0.5 }} onClick={handleEdit} />
        <Popconfirm
          title="确认删除此连接配置？"
          onConfirm={handleDelete}
          onCancel={(e) => e?.stopPropagation()}
          okText="删除"
          cancelText="取消"
        >
          <Button type="text" icon={<DeleteOutlined />} size="small" danger style={{ opacity: 0.5 }}
            onClick={(e) => e.stopPropagation()} />
        </Popconfirm>
      </Space>
    </div>
  );
};

export default ConnectionCard;