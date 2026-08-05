import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import * as api from "../lib/api";
import {
  assetOperationPlanFixture,
  consumptionInventoryFixture,
} from "../test/consumptionFixtures";
import { useConsumptionState } from "./useConsumptionState";

vi.mock("../lib/api", () => ({
  listConsumptionInventory: vi.fn(),
  listAgentCapabilities: vi.fn(),
  planOperation: vi.fn(),
  commitOperation: vi.fn(),
  cancelOperation: vi.fn(),
}));

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(api.listConsumptionInventory).mockResolvedValue(consumptionInventoryFixture());
  vi.mocked(api.listAgentCapabilities).mockResolvedValue([]);
  vi.mocked(api.planOperation).mockResolvedValue({
    domain: "asset",
    plan: assetOperationPlanFixture(),
  });
  vi.mocked(api.commitOperation).mockResolvedValue({
    domain: "asset",
    inventory: consumptionInventoryFixture(),
  });
  vi.mocked(api.cancelOperation).mockResolvedValue(undefined);
});

it("owns central update and delete plans through the same operation state", async () => {
  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.loading).toBe(false));

  await act(async () => {
    await result.current.planUpdate({
      domain: "model",
      existing_id: "work",
      profile: {
        id: "work",
        name: "Work",
        provider: "custom",
        protocol: "openai-responses",
        base_url: "https://example.invalid",
        model: "gpt",
        reasoning: false,
      },
    });
  });
  expect(api.planOperation).toHaveBeenCalledWith({
    operation: "update_central_asset",
    request: {
      draft: expect.objectContaining({ domain: "model", existing_id: "work" }),
    },
  });
  await act(async () => result.current.cancel());

  await act(async () => {
    await result.current.planDelete({ domain: "mcp", key: "local::stdio" }, "manual");
  });
  expect(api.planOperation).toHaveBeenLastCalledWith({
    operation: "delete_central_asset",
    request: {
      asset: { domain: "mcp", key: "local::stdio" },
      source_id: "manual",
    },
  });
});

it("loads inventory and owns one reviewed operation", async () => {
  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.loading).toBe(false));

  await act(async () => {
    await result.current.planForAgent("claude-code", {
      domain: "mcp",
      asset_keys: ["github::stdio", "filesystem::stdio"],
    });
  });
  expect(result.current.plan?.candidate_hash).toBe("candidate");

  await act(async () => {
    await result.current.commit();
  });
  expect(api.commitOperation).toHaveBeenCalledWith({
    domain: "asset",
    request: {
      operation_id: "00000000-0000-4000-8000-000000000001",
      candidate_hash: "candidate",
    },
  });
  expect(result.current.plan).toBeNull();
});

it("plans Agent additions as an atomic delta instead of replacing a stale selection", async () => {
  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.loading).toBe(false));

  await act(async () => {
    await result.current.planAdditionsForAgent("codex", {
      domain: "mcp",
      asset_keys: ["github::stdio"],
    });
  });

  expect(api.planOperation).toHaveBeenCalledWith({
    operation: "ensure_agent_consumption",
    request: {
      agent_id: "codex",
      selection: {
        domain: "mcp",
        asset_keys: ["github::stdio"],
      },
    },
  });
});

it("plans a target-scoped clear for every MCP in one Agent", async () => {
  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.loading).toBe(false));

  await act(async () => {
    await result.current.planClearAgentMcp("qoder");
  });

  expect(api.planOperation).toHaveBeenCalledWith({
    operation: "clear_agent_mcp",
    request: { agent_id: "qoder" },
  });
});

it("loads the Agent capability projection independently from relationships", async () => {
  vi.mocked(api.listAgentCapabilities).mockResolvedValueOnce([{
      identity: {
        id: "model-only",
        name: "Model Only",
        enabled: true,
        builtin: true,
        category: "coding-agent",
        evidence: "official",
      },
      installed: true,
      capabilities: {
        model: {
          mode: "managed",
          installed: true,
          config_paths: ["~/.model-only/config.json"],
          assigned_profiles: [],
          active_profile: null,
          supports_multiple: false,
          credential_mode: "guided",
          supported_protocols: ["openai-responses"],
        },
      },
    }]);

  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.loading).toBe(false));
  expect(result.current.agents.map((agent) => agent.identity.id)).toEqual([
    "model-only",
  ]);
});

it("keeps Agent navigation available when relationship observation fails", async () => {
  vi.mocked(api.listConsumptionInventory).mockRejectedValueOnce({
    code: "model_observation_unavailable",
    message: "one Model config changed during the read",
  });
  vi.mocked(api.listAgentCapabilities).mockResolvedValueOnce([{
    identity: {
      id: "codex",
      name: "Codex",
      enabled: true,
      builtin: true,
      category: "coding-agent",
      evidence: "official",
    },
    installed: true,
    capabilities: {},
  }]);

  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.loading).toBe(false));
  await waitFor(() => expect(result.current.agents).toHaveLength(1));

  expect(result.current.agents[0].identity.id).toBe("codex");
  expect(result.current.inventory).toBeNull();
  expect(result.current.error?.code).toBe("model_observation_unavailable");
  expect(result.current.agentsError).toBeNull();
});

it("retains the last successful relationship snapshot across a failed refresh", async () => {
  const first = consumptionInventoryFixture();
  vi.mocked(api.listConsumptionInventory)
    .mockResolvedValueOnce(first)
    .mockRejectedValueOnce({ code: "scan_failed", message: "temporary failure" });

  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.inventory).toEqual(first));
  await act(async () => {
    await expect(result.current.refresh()).rejects.toEqual({
      code: "scan_failed",
      message: "temporary failure",
    });
  });

  expect(result.current.inventory).toEqual(first);
  expect(result.current.error?.code).toBe("scan_failed");
});

it("refreshes and retries convergence once when the observation is stale", async () => {
  const first = consumptionInventoryFixture();
  const refreshed = { ...consumptionInventoryFixture(), revision: "refreshed-revision" };
  vi.mocked(api.listConsumptionInventory)
    .mockResolvedValueOnce(first)
    .mockResolvedValueOnce(refreshed);
  vi.mocked(api.planOperation)
    .mockRejectedValueOnce({ code: "observation_stale", message: "changed" })
    .mockResolvedValueOnce({ domain: "asset", plan: assetOperationPlanFixture() });
  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.inventory?.revision).toBe("fixture-revision"));
  const item = result.current.inventory!.consumptions[1];

  await act(async () => {
    await result.current.planConvergence(item, "restore-desired");
  });

  expect(api.planOperation).toHaveBeenNthCalledWith(1, {
    operation: "converge_consumption",
    request: expect.objectContaining({ observed_revision: "fixture-revision" }),
  });
  expect(api.planOperation).toHaveBeenNthCalledWith(2, {
    operation: "converge_consumption",
    request: expect.objectContaining({ observed_revision: "refreshed-revision" }),
  });
  expect(result.current.inventory?.revision).toBe("refreshed-revision");
  expect(result.current.error).toBeNull();
});

it("owns MCP enabled-state plans through the central operation slot", async () => {
  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.loading).toBe(false));

  await act(async () => {
    await result.current.planMcpEnabled("codex", "github::stdio", false);
  });

  expect(api.planOperation).toHaveBeenCalledWith({
    operation: "set_mcp_enabled",
    request: {
      agent_id: "codex",
      asset_key: "github::stdio",
      enabled: false,
    },
  });
  expect(result.current.plan?.candidate_hash).toBe("candidate");
});

it("owns Skill enabled-state plans through the central operation slot", async () => {
  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.loading).toBe(false));

  await act(async () => {
    await result.current.planSkillEnabled("codex", "review-changes", false);
  });

  expect(api.planOperation).toHaveBeenCalledWith({
    operation: "set_skill_enabled",
    request: {
      agent_id: "codex",
      name: "review-changes",
      enabled: false,
    },
  });
  expect(result.current.plan?.candidate_hash).toBe("candidate");
});

it("owns Model enabled and active plans through the central operation slot", async () => {
  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.loading).toBe(false));

  await act(async () => {
    await result.current.planModelEnabled("pi", "work", false);
  });
  expect(api.planOperation).toHaveBeenCalledWith({
    operation: "set_model_enabled",
    request: { agent_id: "pi", profile_id: "work", enabled: false },
  });

  await act(async () => result.current.cancel());
  await act(async () => {
    await result.current.planActiveModel("pi", "personal");
  });
  expect(api.planOperation).toHaveBeenLastCalledWith({
    operation: "set_active_model",
    request: { agent_id: "pi", profile_id: "personal" },
  });
});

it("cancels the exact active operation", async () => {
  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.inventory).not.toBeNull());
  await act(async () => {
    await result.current.planForAgent("claude-code", {
      domain: "mcp",
      asset_keys: [],
    });
  });
  await act(async () => {
    await result.current.cancel();
  });
  expect(api.cancelOperation).toHaveBeenCalledWith({
    domain: "asset",
    operation_id: "00000000-0000-4000-8000-000000000001",
  });
  expect(result.current.plan).toBeNull();
});

it("reserves the operation slot while a plan request is still in flight", async () => {
  let resolvePlan!: (plan: ReturnType<typeof assetOperationPlanFixture>) => void;
  vi.mocked(api.planOperation).mockImplementation(
    () => new Promise<ReturnType<typeof assetOperationPlanFixture>>((resolve) => {
      resolvePlan = resolve;
    }).then((plan) => ({ domain: "asset" as const, plan })),
  );
  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.loading).toBe(false));

  const first = result.current.planForAgent("claude-code", {
    domain: "mcp",
    asset_keys: ["github::stdio"],
  });
  await expect(result.current.planForAgent("claude-code", {
    domain: "mcp",
    asset_keys: ["filesystem::stdio"],
  })).rejects.toThrow("已有待确认的资产操作");
  resolvePlan(assetOperationPlanFixture());
  await act(async () => {
    await first;
  });

  expect(api.planOperation).toHaveBeenCalledOnce();
  expect(result.current.plan?.candidate_hash).toBe("candidate");
});
