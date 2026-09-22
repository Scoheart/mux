import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import * as api from "../lib/api";
import { useInstallState } from "./useInstallState";

vi.mock("../lib/api", () => ({
  getRegistrySnapshot: vi.fn(),
  listAgents: vi.fn(),
  scanInstalled: vi.fn(),
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
  vi.mocked(api.getRegistrySnapshot).mockResolvedValue({ entries: [], catalog: [], sources: [], custom_keys: [] });
  vi.mocked(api.listAgents).mockResolvedValue([]);
  vi.mocked(api.scanInstalled).mockResolvedValue([]);
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
  expect(api.getRegistrySnapshot).toHaveBeenCalledOnce();
  expect(api.listSources).not.toHaveBeenCalled();
});

it("lets the startup coordinator progressively load the fresh MCP registry", async () => {
  let resolveRegistry!: (value: Awaited<ReturnType<typeof api.getRegistrySnapshot>>) => void;
  vi.mocked(api.getRegistrySnapshot).mockImplementationOnce(
    () => new Promise((resolve) => {
      resolveRegistry = resolve;
    }),
  );
  const { result } = renderHook(() => useInstallState({ autoLoad: false }));

  expect(api.getRegistrySnapshot).not.toHaveBeenCalled();
  expect(api.scanInstalled).not.toHaveBeenCalled();
  let refresh!: Promise<unknown>;
  act(() => {
    refresh = result.current.refreshRegistry();
  });
  expect(api.getRegistrySnapshot).toHaveBeenCalledOnce();
  expect(result.current.loading).toBe(true);

  await act(async () => resolveRegistry({ entries: [], catalog: [], sources: [], custom_keys: ["fixture::stdio"] }));
  await act(async () => refresh);
  expect(result.current.customKeys.has("fixture::stdio")).toBe(true);
  expect(result.current.loading).toBe(false);
  expect(api.scanInstalled).not.toHaveBeenCalled();
});

it("does not replace a newer observation with a late background response", async () => {
  let finish!: (value: Awaited<ReturnType<typeof api.getRegistrySnapshot>>) => void;
  vi.mocked(api.getRegistrySnapshot).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  const { result } = renderHook(() => useInstallState({ autoLoad: false }));
  let old!: Promise<unknown>;
  act(() => { old = result.current.refreshRegistry(); });
  await act(async () => { await result.current.refreshRegistry(); });
  await act(async () => {
    finish({ entries: [], catalog: [], sources: [], custom_keys: ["stale::stdio"] });
    await old;
  });
  expect(result.current.customKeys.size).toBe(0);
});
