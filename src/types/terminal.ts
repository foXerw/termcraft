export interface TerminalTab {
  id: string;
  connectionId: string;
  title: string;
  connType: 'SSH' | 'Telnet' | 'LocalShell';
  alive: boolean;
}

export interface AppSettings {
  theme: string;
  font_size: number;
  font_family: string;
  default_cols: number;
  default_rows: number;
  scrollback: number;
  cursor_style: string;
  locale: string;
}