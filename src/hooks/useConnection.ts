import { useCallback } from "react";
import { AuthConfig, SerialConfig } from "../types/connection";
import { useAppStore } from "../stores/appStore";

export function useConnection() {
  const addTab = useAppStore((s) => s.addTab);

  const connectSSH = useCallback(async (
    name: string,
    host: string,
    port: number,
    username: string,
    auth: AuthConfig,
  ) => {
    const { invoke, Channel } = await import("@tauri-apps/api/core");
    const id = crypto.randomUUID();
    const channel = new Channel<string>();

    await invoke("connect_ssh", { id, name, host, port, username, auth, channel });

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
    name: string,
    host: string,
    port: number,
  ) => {
    const { invoke, Channel } = await import("@tauri-apps/api/core");
    const id = crypto.randomUUID();
    const channel = new Channel<string>();

    await invoke("connect_telnet", { id, name, host, port, channel });

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
    name: string,
    shell?: string,
  ) => {
    const { invoke, Channel } = await import("@tauri-apps/api/core");
    const id = crypto.randomUUID();
    const channel = new Channel<string>();

    await invoke("connect_local", { id, name, shell: shell || null, channel });

    addTab({
      id,
      connectionId: id,
      title: name || "本地 Shell",
      connType: "LocalShell",
      alive: true,
    });

    return { id, channel };
  }, [addTab]);

  const connectSerial = useCallback(async (
    name: string,
    serial: SerialConfig,
  ) => {
    const { invoke, Channel } = await import("@tauri-apps/api/core");
    const id = crypto.randomUUID();
    const channel = new Channel<string>();

    await invoke("connect_serial", { id, name, config: serial, channel });

    addTab({
      id,
      connectionId: id,
      title: name || serial.port_path || "串口",
      connType: "Serial",
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
    connectSerial,
    disconnect,
    writeToConnection,
    resizeConnection,
  };
}