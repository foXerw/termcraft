export interface ConnectionConfig {
  id: string;
  name: string;
  conn_type: 'SSH' | 'Telnet' | 'LocalShell' | 'Serial';
  host?: string;
  port?: number;
  username?: string;
  auth?: AuthConfig;
  shell?: string;
  serial?: SerialConfig;
  tags: string[];
}

export type AuthConfig =
  | { type: 'Password'; password: string }
  | { type: 'PublicKey'; key_path: string; passphrase?: string }
  | { type: 'Agent' };

export type SerialDataBits = 'Five' | 'Six' | 'Seven' | 'Eight';
export type SerialParity = 'None' | 'Odd' | 'Even';
export type SerialStopBits = 'One' | 'Two';
export type SerialFlowControl = 'None' | 'Software';

/** Serial line framing config (baud/data/parity/stop/flow). Present only on
 *  `conn_type: 'Serial'` connections. Mirrors the Rust `SerialConfig` with
 *  PascalCase variant strings to match serde `rename_all`. */
export interface SerialConfig {
  port_path: string;
  baud_rate: number;
  data_bits: SerialDataBits;
  parity: SerialParity;
  stop_bits: SerialStopBits;
  flow_control: SerialFlowControl;
}

/** 9600 8N1, no flow control — the conservative serial default. `port_path`
 *  is blank until the user picks a port from the dropdown. */
export const DEFAULT_SERIAL_CONFIG: SerialConfig = {
  port_path: '',
  baud_rate: 9600,
  data_bits: 'Eight',
  parity: 'None',
  stop_bits: 'One',
  flow_control: 'None',
};

export interface ConnectionInfo {
  id: string;
  name: string;
  conn_type: 'SSH' | 'Telnet' | 'LocalShell' | 'Serial';
  alive: boolean;
}

/** Reachability probe outcome, pushed from the backend via `connection_status`. */
export type ReachStatus = 'checking' | 'reachable' | 'down' | 'unknown';

export interface ReachState {
  status: ReachStatus;
  latencyMs?: number;
  lastChecked?: string;
}