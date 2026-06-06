import React from "react";
import ConnectionCard from "./ConnectionCard";
import { useConnectionStore } from "../../stores/connectionStore";

const ConnectionList: React.FC = () => {
  const configs = useConnectionStore((s) => s.configs);

  return (
    <div>
      {configs.map((config) => (
        <ConnectionCard key={config.id} config={config} />
      ))}
      {configs.length === 0 && (
        <div style={{ padding: 12, textAlign: "center", color: "var(--text-secondary)" }}>
          暂无保存的连接
        </div>
      )}
    </div>
  );
};

export default ConnectionList;