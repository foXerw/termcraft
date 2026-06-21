import React from "react";
import { Dropdown, message } from "antd";
import { save } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../stores/appStore";

/** Build the default log filename: termcraft-<connType>-<tabTitle>-<timestamp>.log
 *  with illegal filename chars stripped from the title. connType is lowercased
 *  (ssh / telnet / localshell) so the file is easy to grep by connection method. */
function defaultLogName(connType: string, tabTitle: string): string {
  const method = (connType || "unknown").toLowerCase();
  const safe = (tabTitle || "").replace(/[<>:"/\\|?*\x00-\x1f]/g, "").trim() || "terminal";
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const ts = `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}`;
  return `termcraft-${method}-${safe}-${ts}.log`;
}

const TabBar: React.FC = () => {
  const tabs = useAppStore((s) => s.tabs);
  const activeTabId = useAppStore((s) => s.activeTabId);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const removeTab = useAppStore((s) => s.removeTab);
  const logPaths = useAppStore((s) => s.logPaths);
  const setLogPath = useAppStore((s) => s.setLogPath);
  const clearLogPath = useAppStore((s) => s.clearLogPath);

  const startLogging = async (tab: { id: string; connectionId: string; title: string; connType: string }) => {
    const path = await save({
      defaultPath: defaultLogName(tab.connType, tab.title),
      filters: [{ name: "日志文件", extensions: ["log", "txt"] }],
    });
    if (!path) return;
    try {
      await invoke("start_terminal_logging", { id: tab.connectionId, path });
      setLogPath(tab.connectionId, path);
      message.success("开始记录日志");
    } catch (e) {
      message.error(`开始记录失败: ${e}`);
    }
  };

  const stopLogging = async (connectionId: string) => {
    try {
      await invoke("stop_terminal_logging", { id: connectionId });
      clearLogPath(connectionId);
    } catch (e) {
      message.error(`停止记录失败: ${e}`);
    }
  };

  if (tabs.length === 0) {
    return <div className="tab-bar" style={{ flex: 1 }} />;
  }

  return (
    <div className="tab-bar" style={{ flex: 1 }}>
      {tabs.map((tab) => {
        const logging = logPaths.has(tab.connectionId);
        const items = [
          logging
            ? {
                key: "stop",
                label: "停止记录日志",
                onClick: () => stopLogging(tab.connectionId),
              }
            : {
                key: "start",
                label: "开始记录日志",
                onClick: () =>
                  startLogging({ id: tab.id, connectionId: tab.connectionId, title: tab.title, connType: tab.connType }),
              },
        ];
        return (
          <Dropdown key={tab.id} trigger={["contextMenu"]} menu={{ items }}>
            <div
              className={`tab-item ${tab.id === activeTabId ? "active" : ""}`}
              onClick={() => setActiveTab(tab.id)}
              onContextMenu={(e) => e.preventDefault()}
            >
              <span style={{ color: tab.alive ? "var(--success-color)" : "var(--error-color)", fontSize: 10 }}>
                ●
              </span>
              <span>{tab.title}</span>
              <span className="close-btn" onClick={(e) => { e.stopPropagation(); removeTab(tab.id); }}>
                ×
              </span>
            </div>
          </Dropdown>
        );
      })}
    </div>
  );
};

export default TabBar;
