import { create } from 'zustand';
import { ConnectionConfig, ReachState } from '../types/connection';

interface ConnectionState {
  configs: ConnectionConfig[];
  selectedConfigId: string | null;
  /** Per-connection reachability status, keyed by connection id. */
  statusMap: Record<string, ReachState>;

  setConfigs: (configs: ConnectionConfig[]) => void;
  addConfig: (config: ConnectionConfig) => void;
  updateConfig: (config: ConnectionConfig) => void;
  removeConfig: (id: string) => void;
  selectConfig: (id: string | null) => void;
  setReachStatus: (id: string, state: ReachState) => void;
}

export const useConnectionStore = create<ConnectionState>((set) => ({
  configs: [],
  selectedConfigId: null,
  statusMap: {},

  setConfigs: (configs) => set({ configs }),

  addConfig: (config) =>
    set((state) => ({ configs: [...state.configs, config] })),

  updateConfig: (config) =>
    set((state) => ({
      configs: state.configs.map((c) =>
        c.id === config.id ? config : c
      ),
    })),

  removeConfig: (id) =>
    set((state) => ({
      configs: state.configs.filter((c) => c.id !== id),
    })),

  selectConfig: (id) => set({ selectedConfigId: id }),

  setReachStatus: (id, reachState) =>
    set((state) => ({
      statusMap: { ...state.statusMap, [id]: reachState },
    })),
}));