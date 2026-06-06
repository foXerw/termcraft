import React from "react";
import { useAppStore } from "../../stores/appStore";

const TabBar: React.FC = () => {
  const tabs = useAppStore((s) => s.tabs);
  const activeTabId = useAppStore((s) => s.activeTabId);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const removeTab = useAppStore((s) => s.removeTab);

  if (tabs.length === 0) {
    return <div className="tab-bar" style={{ flex: 1 }} />;
  }

  return (
    <div className="tab-bar" style={{ flex: 1 }}>
      {tabs.map((tab) => (
        <div
          key={tab.id}
          className={`tab-item ${tab.id === activeTabId ? "active" : ""}`}
          onClick={() => setActiveTab(tab.id)}
        >
          <span style={{ color: tab.alive ? "var(--success-color)" : "var(--error-color)", fontSize: 10 }}>
            ●
          </span>
          <span>{tab.title}</span>
          <span className="close-btn" onClick={(e) => { e.stopPropagation(); removeTab(tab.id); }}>
            ×
          </span>
        </div>
      ))}
    </div>
  );
};

export default TabBar;