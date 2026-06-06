import React, { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { useAppStore } from "../../stores/appStore";

interface TerminalViewProps {
  connectionId: string;
  tabId: string;
}

const TerminalView: React.FC<TerminalViewProps> = ({ connectionId, tabId }) => {
  const terminalRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const settings = useAppStore((s) => s.settings);
  const activeTabId = useAppStore((s) => s.activeTabId);
  const isActive = activeTabId === tabId;

  // Initialize xterm.js
  useEffect(() => {
    if (!terminalRef.current) return;

    const terminal = new Terminal({
      fontSize: settings.font_size,
      fontFamily: settings.font_family,
      cols: settings.default_cols,
      rows: settings.default_rows,
      scrollback: settings.scrollback,
      cursorStyle: settings.cursor_style as any,
      theme: settings.theme === "dark" ? {
        background: "#1e1e1e",
        foreground: "#cccccc",
        cursor: "#ffffff",
        selectionBackground: "#264f78",
      } : {
        background: "#ffffff",
        foreground: "#333333",
        cursor: "#333333",
        selectionBackground: "#add6ff",
      },
    });

    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(terminalRef.current);
    fitAddon.fit();

    // Try WebGL renderer
    try {
      const webglAddon = new WebglAddon();
      webglAddon.onContextLoss(() => {
        webglAddon.dispose();
      });
      terminal.loadAddon(webglAddon);
    } catch (e) {
      console.warn("WebGL renderer not available, using canvas fallback");
    }

    xtermRef.current = terminal;
    fitAddonRef.current = fitAddon;

    // Handle user input → send to Rust backend
    terminal.onData(async (data) => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("write_to_connection", {
          id: connectionId,
          data,
        });
      } catch (e) {
        console.error("Failed to write to connection:", e);
      }
    });

    // Handle resize → notify Rust backend
    terminal.onResize(async ({ cols, rows }) => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("resize_connection", {
          id: connectionId,
          cols,
          rows,
        });
      } catch (e) {
        console.error("Failed to resize:", e);
      }
    });

    return () => {
      terminal.dispose();
      xtermRef.current = null;
      fitAddonRef.current = null;
    };
  }, [connectionId, settings.font_size, settings.font_family, settings.scrollback]);

  // Bind the Tauri Channel's onmessage to xterm.write
  // The Channel was created in ConnectionForm and stored in appStore.channels
  useEffect(() => {
    const terminal = xtermRef.current;
    if (!terminal) return;

    const channels = useAppStore.getState().channels;
    const channel = channels.get(connectionId);
    if (!channel) {
      console.warn("No channel found for connection", connectionId);
      return;
    }

    // Bind channel.onmessage to write data into xterm
    // Rust sends InvokeResponseBody::Json(text) which arrives as the parsed value
    channel.onmessage = (msg: any) => {
      if (xtermRef.current) {
        // msg could be a string (raw terminal output) or a parsed JSON object
        // Rust sends: InvokeResponseBody::Json(serde_json::to_string(&text))
        // Frontend receives the deserialized value
        const text = typeof msg === 'string' ? msg : String(msg);
        xtermRef.current.write(text);
      }
    };

    console.log("TerminalView bound channel for", connectionId);
  }, [connectionId, xtermRef.current]);

  // Fit terminal when tab becomes active
  useEffect(() => {
    if (isActive && fitAddonRef.current && xtermRef.current) {
      setTimeout(() => {
        fitAddonRef.current?.fit();
      }, 50);
    }
  }, [isActive]);

  // Resize on window resize
  useEffect(() => {
    const handleResize = () => {
      if (isActive && fitAddonRef.current) {
        fitAddonRef.current.fit();
      }
    };
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, [isActive]);

  return (
    <div
      className="terminal-wrapper"
      ref={terminalRef}
      style={{
        display: isActive ? "block" : "none",
        width: "100%",
        height: "100%",
      }}
    />
  );
};

export default TerminalView;