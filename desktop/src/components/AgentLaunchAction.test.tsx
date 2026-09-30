import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { AgentLauncherContext } from "../lib/agentLauncherContext";
import { getAgentLaunchInfo, getAgentRuntimeVersion } from "../lib/agentLaunch";
import { AgentLaunchAction } from "./AgentLaunchAction";
import { ToastProvider } from "./Toast";

vi.mock("../lib/agentLaunch", () => ({
  getAgentLaunchInfo: vi.fn(),
  getAgentRuntimeVersion: vi.fn(),
}));

afterEach(() => { cleanup(); vi.clearAllMocks(); });

it("shows installed status immediately and adds the version when the separate probe completes", async () => {
  let completeVersion!: (version: string | null) => void;
  vi.mocked(getAgentLaunchInfo).mockResolvedValue({
    agent_id: "pi", name: "Pi Coding Agent", category: "cli", kind: "cli",
    available: true, supported: true, host_name: null, install_url: null,
    directory: null, directory_exists: false, configured_target: null,
    resolved_target: { kind: "cli", command: "/usr/local/bin/pi", args: [] },
  });
  vi.mocked(getAgentRuntimeVersion).mockReturnValue(new Promise((resolve) => { completeVersion = resolve; }));

  render(<ToastProvider><AgentLauncherContext.Provider value={{
    busyId: null, revision: 0, launch: vi.fn(), configure: vi.fn(), refresh: vi.fn(),
  }}><AgentLaunchAction agentId="pi" showLabel /></AgentLauncherContext.Provider></ToastProvider>);

  await waitFor(() => expect(screen.getByRole("status", { name: "Pi Coding Agent 已安装，版本未知" })).toBeVisible());
  expect(screen.getByRole("button", { name: "打开 Agent" })).toBeEnabled();
  await act(async () => completeVersion("0.99.0"));
  expect(await screen.findByRole("status", { name: "Pi Coding Agent 已安装，版本 0.99.0" })).toHaveTextContent("v0.99.0");
  expect(screen.getByRole("status")).toHaveAttribute("title", expect.stringContaining("/usr/local/bin/pi"));
});
