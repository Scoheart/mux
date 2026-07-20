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
  planSetAgentConsumption: vi.fn(),
  planSetActiveModel: vi.fn(),
  planSetMcpEnabled: vi.fn(),
  planSetModelEnabled: vi.fn(),
  planSetAssetConsumers: vi.fn(),
  planUpdateCentralAsset: vi.fn(),
  planDeleteCentralAsset: vi.fn(),
  commitAssetOperation: vi.fn(),
  cancelAssetOperation: vi.fn(),
}));

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(api.listConsumptionInventory).mockResolvedValue(
    consumptionInventoryFixture(),
  );
  vi.mocked(api.planSetAgentConsumption).mockResolvedValue(
    assetOperationPlanFixture(),
  );
  vi.mocked(api.planSetMcpEnabled).mockResolvedValue(
    assetOperationPlanFixture(),
  );
  vi.mocked(api.planSetModelEnabled).mockResolvedValue(
    assetOperationPlanFixture(),
  );
  vi.mocked(api.planSetActiveModel).mockResolvedValue(
    assetOperationPlanFixture(),
  );
  vi.mocked(api.planUpdateCentralAsset).mockResolvedValue(
    assetOperationPlanFixture(),
  );
  vi.mocked(api.planDeleteCentralAsset).mockResolvedValue(
    assetOperationPlanFixture(),
  );
  vi.mocked(api.commitAssetOperation).mockResolvedValue(
    consumptionInventoryFixture(),
  );
  vi.mocked(api.cancelAssetOperation).mockResolvedValue(undefined);
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
        protocol: "openai-responses",
        base_url: "https://example.invalid",
        model: "gpt",
        reasoning: false,
      },
    });
  });
  expect(api.planUpdateCentralAsset).toHaveBeenCalledOnce();
  await act(async () => result.current.cancel());

  await act(async () => {
    await result.current.planDelete({ domain: "mcp", key: "local::stdio" }, "manual");
  });
  expect(api.planDeleteCentralAsset).toHaveBeenCalledWith(
    { domain: "mcp", key: "local::stdio" },
    "manual",
  );
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
  expect(api.commitAssetOperation).toHaveBeenCalledWith(
    assetOperationPlanFixture(),
    undefined,
  );
  expect(result.current.plan).toBeNull();
});

it("owns MCP enabled-state plans through the central operation slot", async () => {
  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.loading).toBe(false));

  await act(async () => {
    await result.current.planMcpEnabled("codex", "github::stdio", false);
  });

  expect(api.planSetMcpEnabled).toHaveBeenCalledWith("codex", "github::stdio", false);
  expect(result.current.plan?.candidate_hash).toBe("candidate");
});

it("owns Model enabled and active plans through the central operation slot", async () => {
  const { result } = renderHook(() => useConsumptionState());
  await waitFor(() => expect(result.current.loading).toBe(false));

  await act(async () => {
    await result.current.planModelEnabled("pi", "work", false);
  });
  expect(api.planSetModelEnabled).toHaveBeenCalledWith("pi", "work", false);

  await act(async () => result.current.cancel());
  await act(async () => {
    await result.current.planActiveModel("pi", "personal");
  });
  expect(api.planSetActiveModel).toHaveBeenCalledWith("pi", "personal");
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
  expect(api.cancelAssetOperation).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000001",
  );
  expect(result.current.plan).toBeNull();
});

it("reserves the operation slot while a plan request is still in flight", async () => {
  let resolvePlan!: (plan: ReturnType<typeof assetOperationPlanFixture>) => void;
  vi.mocked(api.planSetAgentConsumption).mockImplementation(
    () => new Promise((resolve) => {
      resolvePlan = resolve;
    }),
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

  expect(api.planSetAgentConsumption).toHaveBeenCalledOnce();
  expect(result.current.plan?.candidate_hash).toBe("candidate");
});
