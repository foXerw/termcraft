import { create } from 'zustand';
import { Preset, PresetGroup, PresetExecutionStatus } from '../types/preset';

interface PresetState {
  presets: Preset[];
  groups: PresetGroup[];
  executionStatuses: Record<string, PresetExecutionStatus>;

  setPresets: (presets: Preset[]) => void;
  setGroups: (groups: PresetGroup[]) => void;
  addPreset: (preset: Preset) => void;
  updatePreset: (preset: Preset) => void;
  removePreset: (id: string) => void;
  addGroup: (group: PresetGroup) => void;
  removeGroup: (id: string) => void;
  updateExecutionStatus: (execId: string, status: PresetExecutionStatus) => void;
  clearExecutionStatus: (execId: string) => void;
}

export const usePresetStore = create<PresetState>((set) => ({
  presets: [],
  groups: [],
  executionStatuses: {},

  setPresets: (presets) => set({ presets }),
  setGroups: (groups) => set({ groups }),

  addPreset: (preset) =>
    set((state) => ({ presets: [...state.presets, preset] })),

  updatePreset: (preset) =>
    set((state) => ({
      presets: state.presets.map((p) =>
        p.id === preset.id ? preset : p
      ),
    })),

  removePreset: (id) =>
    set((state) => ({
      presets: state.presets.filter((p) => p.id !== id),
    })),

  addGroup: (group) =>
    set((state) => ({ groups: [...state.groups, group] })),

  removeGroup: (id) =>
    set((state) => ({
      groups: state.groups.filter((g) => g.id !== id),
    })),

  updateExecutionStatus: (execId, status) =>
    set((state) => ({
      executionStatuses: {
        ...state.executionStatuses,
        [execId]: status,
      },
    })),

  clearExecutionStatus: (execId) =>
    set((state) => {
      const newStatuses = { ...state.executionStatuses };
      delete newStatuses[execId];
      return { executionStatuses: newStatuses };
    }),
}));