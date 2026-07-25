import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import * as api from "../lib/api";
import type { RegistryEntry } from "../lib/types";
import { useInstallState } from "./useInstallState";

vi.mock("../lib/api", () => ({
  listRegistry: vi.fn(),
  listRegistryAll: vi.fn(),
  listAgents: vi.fn(),
  scanInstalled: vi.fn(),
  listCustomRegistryKeys: vi.fn(),
  listSources: vi.fn(),
  subscribeSource: vi.fn(),
  addLocalSourceDialog: vi.fn(),
  refreshSource: vi.fn(),
  setSourceEnabled: vi.fn(),
  removeSource: vi.fn(),
  importPastedConfig: vi.fn(),
}));

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(api.listRegistry).mockResolvedValue([]);
  vi.mocked(api.listRegistryAll).mockResolvedValue([]);
  vi.mocked(api.listAgents).mockResolvedValue([]);
  vi.mocked(api.scanInstalled).mockResolvedValue([]);
  vi.mocked(api.listCustomRegistryKeys).mockResolvedValue([]);
  vi.mocked(api.listSources).mockResolvedValue([]);
});

it("refreshes observed state without importing discovered MCPs", async () => {
  const { result } = renderHook(() => useInstallState());
  await waitFor(() => expect(result.current.loading).toBe(false));
  vi.clearAllMocks();

  await act(async () => {
    await result.current.refreshAll();
  });

  expect(api.scanInstalled).toHaveBeenCalledOnce();
  expect(api.listRegistry).toHaveBeenCalledOnce();
  expect(api.listSources).toHaveBeenCalledOnce();
});

it("lets the startup coordinator progressively load the fresh MCP registry", async () => {
  let resolveRegistry!: (value: RegistryEntry[]) => void;
  vi.mocked(api.listRegistry).mockImplementationOnce(
    () => new Promise((resolve) => {
      resolveRegistry = resolve;
    }),
  );
  const { result } = renderHook(() => useInstallState({ autoLoad: false }));

  expect(api.listRegistry).not.toHaveBeenCalled();
  expect(api.scanInstalled).not.toHaveBeenCalled();
  let refresh!: Promise<unknown>;
  act(() => {
    refresh = result.current.refreshRegistry();
  });
  expect(api.listRegistry).toHaveBeenCalledOnce();
  expect(api.listRegistryAll).not.toHaveBeenCalled();
  expect(result.current.loading).toBe(true);

  await act(async () => resolveRegistry([]));
  await act(async () => refresh);
  expect(api.listRegistryAll).toHaveBeenCalledOnce();
  expect(api.listCustomRegistryKeys).toHaveBeenCalledOnce();
  expect(result.current.loading).toBe(false);
  expect(api.scanInstalled).not.toHaveBeenCalled();
});
