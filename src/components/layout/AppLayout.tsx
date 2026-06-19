import React from "react";
import { Button } from "antd";
import { MenuFoldOutlined, MenuUnfoldOutlined, PlusOutlined } from "@ant-design/icons";
import Sidebar from "./Sidebar";
import TabBar from "./TabBar";
import TerminalManager from "../terminal/TerminalManager";
import ConnectionForm from "../connection/ConnectionForm";
import { useAppStore } from "../../stores/appStore";

const AppLayout: React.FC = () => {
  const sidebarCollapsed = useAppStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);
  const connectionFormOpen = useAppStore((s) => s.connectionFormOpen);
  const editingConfig = useAppStore((s) => s.editingConfig);
  const openConnectionForm = useAppStore((s) => s.openConnectionForm);
  const closeConnectionForm = useAppStore((s) => s.closeConnectionForm);

  return (
    <div className="app-layout">
      <div className="main-area">
        <Sidebar collapsed={sidebarCollapsed} />
        <div className="content-area">
          <div style={{ display: "flex", alignItems: "center", padding: "0 8px", background: "var(--bg-secondary)", borderBottom: "1px solid var(--border-color)" }}>
            <Button
              type="text"
              icon={sidebarCollapsed ? <MenuUnfoldOutlined /> : <MenuFoldOutlined />}
              onClick={toggleSidebar}
              size="small"
            />
            <TabBar />
            <Button
              type="text"
              icon={<PlusOutlined />}
              size="small"
              style={{ marginLeft: 8 }}
              onClick={() => openConnectionForm()}
            />
          </div>
          <TerminalManager />
        </div>
      </div>

      {/* Connection form dialog */}
      <ConnectionForm open={connectionFormOpen} initialValues={editingConfig ?? undefined} onCancel={closeConnectionForm} />
    </div>
  );
};

export default AppLayout;