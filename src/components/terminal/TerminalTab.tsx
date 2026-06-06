import React from "react";
import TerminalView from "./TerminalView";

interface TerminalTabProps {
  id: string;
  connectionId: string;
  isActive: boolean;
}

const TerminalTab: React.FC<TerminalTabProps> = ({ id, connectionId, isActive }) => {
  return (
    <div style={{ display: isActive ? "block" : "none", width: "100%", height: "100%" }}>
      <TerminalView connectionId={connectionId} tabId={id} />
    </div>
  );
};

export default TerminalTab;