import React from "react";
import TerminalView from "./TerminalView";
import { useAppStore } from "../../stores/appStore";
import { Typography } from "antd";

const TerminalManager: React.FC = () => {
  const tabs = useAppStore((s) => s.tabs);
  const activeTabId = useAppStore((s) => s.activeTabId);

  if (tabs.length === 0) {
    return (
      <div className="terminal-container">
        <div className="no-connection-message">
          <div style={{ textAlign: "center" }}>
            <Typography.Title level={4} style={{ color: "var(--text-secondary)" }}>
              TermCraft
            </Typography.Title>
            <Typography.Text style={{ color: "var(--text-secondary)" }}>
              点击左侧连接列表快速连接，或新建连接开始使用
            </Typography.Text>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="terminal-container">
      {tabs.map((tab) => (
        <TerminalView
          key={tab.id}
          connectionId={tab.connectionId}
          tabId={tab.id}
        />
      ))}
    </div>
  );
};

export default TerminalManager;