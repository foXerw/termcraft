import { useCallback } from "react";
import { ConnectionConfig, AuthConfig } from "../types/connection";
import { useAppStore } from "../stores/appStore";

export function useConnection() {
  const addTab = useAppStore((s) => s.addTab);

  const connectSSH = useCallback(async (
    host: string,
    port: number,
    username: string,
    auth: AuthConfig,
    name?: string,
  ) => {
    const { invoke, Channel } = await import("@tauri-apps/api/core");
    const id = crypto.randomUUID();
    const channel = new Channel<string>();

    await invoke("connect_ssh", { id, host, port, username, auth, channel });

    addTab({
      id,
      connectionId: id,
      title: name || `${host}:${port}`,
      connType: "SSH",
      alive: true,
    });

    return { id, channel };
  }, [addTab]);

  const connectTelnet = useCallback(async (
    host: string,
    port: number,
    name?: string,
  ) => {
    const { invoke, Channel } = await import("@tauri-apps/api/core");
    const id = crypto.randomUUID();
    const channel = new Channel<string>();

    await invoke("connect_telnet", { id, host, port, channel });

    addTab({
      id,
      connectionId: id,
      title: name || `${host}:${port}`,
      connType: "Telnet",
      alive: true,
    });

    return { id, channel };
  }, [addTab]);

  const connectLocal = useCallback(async (
    shell?: string,
    name?: string,
  ) => {
    const { invoke, Channel } = await import("@tauri-apps/api/core");
    const id = crypto.randomUUID();
    const channel = new Channel<string>();

    await invoke("connect_local", { id, shell: shell || null, channel });

    addTab({
      id,
      connectionId: id,
      title: name || "本地 Shell",
      connType: "LocalShell",
      alive: true,
    });

    return { id, channel };
  }, [addTab]);

  const disconnect = useCallback(async (id: string) => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("disconnect", { id });
    useAppStore.getState().updateTabAlive(id, false);
  }, []);

  const writeToConnection = useCallback(async (id: string, data: string) => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("write_to_connection", { id, data });
  }, []);

  const resizeConnection = useCallback(async (id: string, cols: number, rows: number) => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("resize_connection", { id, cols, rows });
  }, []);

  return {
    connectSSH,
    connectTelnet,
    connectLocal,
    disconnect,
    writeToConnection,
    resizeConnection,
  };
}