import { create } from 'zustand';
import { TerminalTab, AppSettings } from '../types/terminal';
import { ConnectionConfig } from '../types/connection';

// Store Tauri Channel instances keyed by connectionId
// This allows TerminalView to pick up the channel created during connection
interface AppState {
  // Active terminal tabs
  tabs: TerminalTab[];
  activeTabId: string | null;

  // App settings
  settings: AppSettings;

  // Sidebar collapsed
  sidebarCollapsed: boolean;

  // Connection form dialog
  connectionFormOpen: boolean;
  // When set, the connection form opens in edit mode pre-filled with this config
  editingConfig: ConnectionConfig | null;

  // Tauri IPC Channels for each connection (connectionId -> Channel)
  channels: Map<string, any>;

  // Active log file path per connection (connectionId -> path). Drives the
  // tab right-click menu state and the terminal bottom status bar.
  logPaths: Map<string, string>;

  // Actions
  addTab: (tab: TerminalTab) => void;
  removeTab: (id: string) => void;
  setActiveTab: (id: string) => void;
  updateTabAlive: (id: string, alive: boolean) => void;
  updateSettings: (settings: AppSettings) => void;
  toggleSidebar: () => void;
  openConnectionForm: (config?: ConnectionConfig) => void;
  closeConnectionForm: () => void;
  setChannel: (connectionId: string, channel: any) => void;
  removeChannel: (connectionId: string) => void;
  setLogPath: (connId: string, path: string) => void;
  clearLogPath: (connId: string) => void;
}

const defaultSettings: AppSettings = {
  theme: 'dark',
  font_size: 14,
  font_family: "Consolas, 'Courier New', monospace",
  default_cols: 80,
  default_rows: 24,
  scrollback: 5000,
  cursor_style: 'block',
  locale: 'zh-CN',
};

export const useAppStore = create<AppState>((set) => ({
  tabs: [],
  activeTabId: null,
  settings: defaultSettings,
  sidebarCollapsed: false,
  connectionFormOpen: false,
  editingConfig: null,
  channels: new Map(),
  logPaths: new Map(),

  addTab: (tab) =>
    set((state) => ({
      tabs: [...state.tabs, tab],
      activeTabId: tab.id,
    })),

  removeTab: (id) =>
    set((state) => {
      const newTabs = state.tabs.filter((t) => t.id !== id);
      const newActiveId =
        state.activeTabId === id
          ? newTabs.length > 0
            ? newTabs[newTabs.length - 1].id
            : null
          : state.activeTabId;
      const newChannels = new Map(state.channels);
      newChannels.delete(id);
      return { tabs: newTabs, activeTabId: newActiveId, channels: newChannels };
    }),

  setActiveTab: (id) => set({ activeTabId: id }),

  updateTabAlive: (id, alive) =>
    set((state) => ({
      tabs: state.tabs.map((t) =>
        t.id === id ? { ...t, alive } : t
      ),
    })),

  updateSettings: (settings) => set({ settings }),

  toggleSidebar: () =>
    set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),

  openConnectionForm: (config) =>
    set({ connectionFormOpen: true, editingConfig: config ?? null }),
  closeConnectionForm: () =>
    set({ connectionFormOpen: false, editingConfig: null }),

  setChannel: (connectionId, channel) =>
    set((state) => {
      const newChannels = new Map(state.channels);
      newChannels.set(connectionId, channel);
      return { channels: newChannels };
    }),

  removeChannel: (connectionId) =>
    set((state) => {
      const newChannels = new Map(state.channels);
      newChannels.delete(connectionId);
      return { channels: newChannels };
    }),

  setLogPath: (connId, path) =>
    set((state) => {
      const next = new Map(state.logPaths);
      next.set(connId, path);
      return { logPaths: next };
    }),

  clearLogPath: (connId) =>
    set((state) => {
      const next = new Map(state.logPaths);
      next.delete(connId);
      return { logPaths: next };
    }),
}));