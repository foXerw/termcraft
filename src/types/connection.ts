export interface ConnectionConfig {
  id: string;
  name: string;
  conn_type: 'SSH' | 'Telnet' | 'LocalShell';
  host?: string;
  port?: number;
  username?: string;
  auth?: AuthConfig;
  shell?: string;
  tags: string[];
}

export type AuthConfig =
  | { type: 'Password'; password: string }
  | { type: 'PublicKey'; key_path: string; passphrase?: string }
  | { type: 'Agent' };

export interface ConnectionInfo {
  id: string;
  name: string;
  conn_type: 'SSH' | 'Telnet' | 'LocalShell';
  alive: boolean;
}

/** Reachability probe outcome, pushed from the backend via `connection_status`. */
export type ReachStatus = 'checking' | 'reachable' | 'down' | 'unknown';

export interface ReachState {
  status: ReachStatus;
  latencyMs?: number;
  lastChecked?: string;
}