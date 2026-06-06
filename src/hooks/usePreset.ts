import { useCallback } from "react";
import { Preset } from "../types/preset";
import { usePresetStore } from "../stores/presetStore";

export function usePreset() {
  const savePreset = useCallback(async (preset: Preset) => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("save_preset", { preset });
    usePresetStore.getState().addPreset(preset);
  }, []);

  const deletePreset = useCallback(async (id: string) => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("delete_preset", { id });
    usePresetStore.getState().removePreset(id);
  }, []);

  const loadPresets = useCallback(async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const presets = await invoke("load_presets") as Preset[];
    usePresetStore.getState().setPresets(presets);
    return presets;
  }, []);

  return {
    savePreset,
    deletePreset,
    loadPresets,
  };
}