import { useEffect, useRef, useCallback } from "react";

export function useChannel() {
  const channelRef = useRef<any>(null);

  const createChannel = useCallback(async () => {
    const { Channel } = await import("@tauri-apps/api/core");
    const channel = new Channel<string>();
    channelRef.current = channel;
    return channel;
  }, []);

  const sendMessage = useCallback(async (data: string) => {
    if (channelRef.current) {
      await channelRef.current.send(data);
    }
  }, []);

  return {
    channel: channelRef,
    createChannel,
    sendMessage,
  };
}