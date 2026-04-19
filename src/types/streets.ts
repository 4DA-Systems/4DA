// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// STREETS Command Execution types — mirrors src-tauri/src/streets_commands.rs

export type OsTarget = 'linux' | 'mac_os' | 'windows' | 'universal';
export type RiskLevel = 'safe' | 'moderate' | 'elevated';

export interface ParsedCommand {
  id: string;
  command: string;
  os_target: OsTarget;
  language: string;
  risk_level: RiskLevel;
  description: string;
}

export interface CommandExecutionResult {
  command_id: string;
  success: boolean;
  stdout: string;
  stderr: string;
  exit_code: number;
  duration_ms: number;
  executed_at: string;
}
