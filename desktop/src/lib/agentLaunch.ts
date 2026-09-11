import { invoke } from "@tauri-apps/api/core";

export type LaunchTarget = { kind: "app"; path: string }
  | { kind: "cli"; command: string; args: string[] }
  | { kind: "web"; url: string };
export interface AgentLaunchInfo {
  agent_id: string;
  name: string;
  kind: "app" | "cli" | "web" | null;
  available: boolean;
  supported: boolean;
  host_name: string | null;
  install_url: string | null;
  directory: string | null;
  directory_exists: boolean;
  configured_target: LaunchTarget | null;
  resolved_target: LaunchTarget | null;
}
export const getAgentLaunchInfo = (agentId: string) => invoke<AgentLaunchInfo>("get_agent_launch_info", { agentId });
export const configureAgentLaunch = (agentId: string, target: LaunchTarget | null) => invoke<AgentLaunchInfo>("configure_agent_launch", { agentId, target });
export const launchAgent = (agentId: string, directory: string | null) => invoke<{ directory_saved: boolean }>("launch_agent", { agentId, directory });
