export interface Preset {
  id: string;
  name: string;
  group_id?: string;
  description?: string;
  commands: CommandItem[];
  variables: Variable[];
  execution_mode: ExecutionMode;
  created_at: string;
  updated_at: string;
}

export interface CommandItem {
  id: string;
  command: string;
  delay_ms: number;
  wait_for?: WaitCondition;
  /** Abort = stop the whole preset on failure; Continue = log and proceed. */
  on_fail?: 'Abort' | 'Continue';
  enabled: boolean;
}

export interface WaitCondition {
  pattern: string;
  timeout_ms: number;
  match_type: 'Exact' | 'Contains' | 'Regex';
  /** Found = pattern appearing means success; NotFound = appearing means failure. */
  expect?: 'Found' | 'NotFound';
}

export type ExecutionMode =
  | { type: 'Single' }
  | { type: 'Batch'; stop_on_error: boolean }
  | { type: 'Loop'; count?: number; interval_ms: number; stop_on_error: boolean };

export interface Variable {
  name: string;
  default_value?: string;
  description?: string;
}

export interface PresetGroup {
  id: string;
  name: string;
  description?: string;
  parent_id?: string;
}

export interface PresetExecutionStatus {
  exec_id: string;
  preset_id: string;
  state: 'Running' | 'Paused' | 'Completed' | 'Failed' | 'Cancelled';
  current_command_index: number;
  total_commands: number;
  current_loop: number;
  message?: string;
  command_succeeded?: boolean;
  captured_snippet?: string;
}

export interface ScheduledTask {
  id: string;
  preset_id: string;
  connection_id: string;
  variables: Record<string, string>;
  schedule: Schedule;
  enabled: boolean;
}

export type Schedule =
  | { type: 'Cron'; expression: string }
  | { type: 'Interval'; seconds: number }
  | { type: 'Once'; at: string };