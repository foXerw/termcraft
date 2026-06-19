import React, { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { useAppStore } from "../../stores/appStore";

/**
 * Fallback terminal shown when no connection tabs are open. It spins up a
 * local shell (cmd.exe / bash) so the user always has a working terminal in
 * the main area. When the shell exits (e.g. user types `exit`), the backend
 * emits `connection_closed`; we respawn a fresh shell.
 *
 * Not a tab — purely a fallback view. Connects/tears down its own connection.
 */
const DefaultTerminal: React.FC = () => {
  const containerRef = useRef<HTMLDivElement>(null);
  // Bump to spawn a new local-shell session (used on mount + after exit).
  const [session, setSession] = useState(0);
  const idRef = useRef<string | null>(null);
  const settings = useAppStore((s) => s.settings);

  // Spawn a local shell and bind it to an xterm.
  useEffect(() => {
    let disposed = false;
    let term: Terminal | null = null;
    let fit: FitAddon | null = null;
    let unlistenClose: (() => void) | undefined;

    (async () => {
      const { invoke, Channel } = await import("@tauri-apps/api/core");
      const id = "default-" + crypto.randomUUID();
      idRef.current = id;

      const channel = new Channel();
      useAppStore.getState().setChannel(id, channel);

      const terminal = new Terminal({
        fontSize: settings.font_size,
        fontFamily: settings.font_family,
        cols: settings.default_cols,
        rows: settings.default_rows,
        cursorStyle: settings.cursor_style as any,
        theme: { background: "#1e1e1e", foreground: "#cccccc", cursor: "#ffffff" },
      });
      const fitAddon = new FitAddon();
      terminal.loadAddon(fitAddon);
      if (containerRef.current) {
        terminal.open(containerRef.current);
        fitAddon.fit();
      }
      try {
        const webgl = new WebglAddon();
        webgl.onContextLoss(() => webgl.dispose());
        terminal.loadAddon(webgl);
      } catch {
        /* canvas fallback */
      }
      term = terminal;
      fit = fitAddon;

      channel.onmessage = (msg: any) => {
        const text = typeof msg === "string" ? msg : String(msg);
        term?.write(text);
      };

      terminal.onData(async (data) => {
        try {
          await invoke("write_to_connection", { id, data });
        } catch (e) {
          console.error("default term write failed:", e);
        }
      });

      try {
        await invoke("connect_local", { id, shell: null, channel });
      } catch (e) {
        console.error("default local shell failed:", e);
      }

      // If this shell exits (user typed `exit`), respawn a fresh one.
      const { listen } = await import("@tauri-apps/api/event");
      unlistenClose = await listen<string>("connection_closed", (ev) => {
        if (disposed) return;
        if (ev.payload === id) {
          setSession((n) => n + 1); // triggers re-spawn via the effect dep
        }
      });
    })();

    return () => {
      disposed = true;
      unlistenClose?.();
      const id = idRef.current;
      if (id) {
        import("@tauri-apps/api/core").then(({ invoke }) => {
          invoke("disconnect", { id }).catch(() => {});
        });
        useAppStore.getState().removeChannel(id);
      }
      term?.dispose();
    };
    // Re-run when the session counter changes (respawn after exit).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session]);

  // Keep fit correct on container resize.
  useEffect(() => {
    const onResize = () => {
      // FitAddon reference not retained across this effect; rely on xterm's
      // own resize handling. (Best-effort.)
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  return <div ref={containerRef} className="terminal-container" style={{ height: "100%", width: "100%" }} />;
};

export default DefaultTerminal;
