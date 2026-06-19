import React, { useEffect } from "react";
import TerminalView from "./TerminalView";
import DefaultTerminal from "./DefaultTerminal";
import { useAppStore } from "../../stores/appStore";

const TerminalManager: React.FC = () => {
  const tabs = useAppStore((s) => s.tabs);
  const removeTab = useAppStore((s) => s.removeTab);

  // When a connection closes (shell exited via `exit` / disconnected / EOF),
  // drop its tab so the default-terminal fallback reappears if it was the
  // last one.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen<string>("connection_closed", (ev) => {
        removeTab(ev.payload);
      });
    })();
    return () => { unlisten?.(); };
  }, [removeTab]);

  if (tabs.length === 0) {
    return <DefaultTerminal />;
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
