import { StrictMode, type ReactNode } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { useStartupSync, type StartupTask } from "./useStartupSync";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

it("bounds foreground reads and starts deferred scans only after they settle", async () => {
  const gates = [deferred(), deferred(), deferred()];
  let active = 0;
  let peak = 0;
  const order: string[] = [];
  const foreground: StartupTask[] = gates.map((gate, index) => ({
    id: `foreground-${index}`,
    label: `foreground-${index}`,
    run: vi.fn(async () => {
      order.push(`start-${index}`);
      active += 1;
      peak = Math.max(peak, active);
      await gate.promise;
      active -= 1;
      order.push(`end-${index}`);
    }),
  }));
  const deferredRun = vi.fn(async () => {
    order.push("deferred");
  });
  const deferredTasks: StartupTask[] = [
    { id: "deferred", label: "deferred", run: deferredRun },
  ];

  const { result } = renderHook(() =>
    useStartupSync({
      foreground,
      deferred: deferredTasks,
      foregroundConcurrency: 2,
      deferredDelayMs: 0,
    }),
  );

  await waitFor(() => expect(foreground[0].run).toHaveBeenCalledOnce());
  expect(foreground[1].run).toHaveBeenCalledOnce();
  expect(foreground[2].run).not.toHaveBeenCalled();
  expect(deferredRun).not.toHaveBeenCalled();

  await act(async () => gates[0].resolve());
  await waitFor(() => expect(foreground[2].run).toHaveBeenCalledOnce());
  expect(peak).toBe(2);
  expect(result.current.completed).toBe(1);

  await act(async () => {
    gates[1].resolve();
    gates[2].resolve();
  });
  await waitFor(() => expect(deferredRun).toHaveBeenCalledOnce());
  await waitFor(() => expect(result.current.settled).toBe(true));
  expect(order.indexOf("deferred")).toBeGreaterThan(order.indexOf("end-2"));
});

it("keeps a failed read non-blocking and retries only failed work", async () => {
  const healthy = vi.fn(async () => undefined);
  const flaky = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(new Error("offline"))
    .mockResolvedValueOnce(undefined);
  const foreground: StartupTask[] = [
    { id: "healthy", label: "healthy", run: healthy },
    { id: "flaky", label: "flaky", run: flaky },
  ];

  const { result } = renderHook(() =>
    useStartupSync({
      foreground,
      deferred: [],
      foregroundConcurrency: 2,
    }),
  );

  await waitFor(() => expect(result.current.settled).toBe(true));
  expect(result.current.failed).toBe(1);
  expect(result.current.completed).toBe(1);

  await act(async () => result.current.retryFailed());
  expect(healthy).toHaveBeenCalledOnce();
  expect(flaky).toHaveBeenCalledTimes(2);
  expect(result.current.failed).toBe(0);
  expect(result.current.completed).toBe(2);
});

it("tracks later observation failures without resetting healthy task state", async () => {
  const agents = vi.fn(async () => undefined);
  const relationships = vi
    .fn<() => Promise<void>>()
    .mockResolvedValueOnce(undefined)
    .mockRejectedValueOnce(new Error("one Agent file changed"))
    .mockResolvedValueOnce(undefined);
  const foreground: StartupTask[] = [
    { id: "agents", label: "agents", run: agents },
    { id: "relationships", label: "relationships", run: relationships },
  ];
  const { result } = renderHook(() =>
    useStartupSync({ foreground, deferred: [] }),
  );
  await waitFor(() => expect(result.current.settled).toBe(true));

  await act(async () => result.current.refreshTasks(["relationships"]));
  expect(agents).toHaveBeenCalledOnce();
  expect(relationships).toHaveBeenCalledTimes(2);
  expect(result.current.completed).toBe(1);
  expect(result.current.failed).toBe(1);
  expect(result.current.tasks.find(({ id }) => id === "agents")?.status).toBe("complete");

  await act(async () => result.current.retryFailed());
  expect(relationships).toHaveBeenCalledTimes(3);
  expect(result.current.completed).toBe(2);
  expect(result.current.failed).toBe(0);
});

it("surfaces a slow state without blocking completed data", async () => {
  const gate = deferred();
  const foreground: StartupTask[] = [
    { id: "slow", label: "slow", run: () => gate.promise },
  ];
  const { result } = renderHook(() =>
    useStartupSync({
      foreground,
      deferred: [],
      slowAfterMs: 10,
    }),
  );

  await waitFor(() => expect(result.current.slow).toBe(true));
  expect(result.current.syncing).toBe(true);
  await act(async () => gate.resolve());
  await waitFor(() => expect(result.current.settled).toBe(true));
  expect(result.current.slow).toBe(false);
});

it("does not duplicate fresh scans under the app's StrictMode root", async () => {
  const run = vi.fn(async () => undefined);
  const foreground: StartupTask[] = [
    { id: "registry", label: "registry", run },
  ];
  const wrapper = ({ children }: { children: ReactNode }) => (
    <StrictMode>{children}</StrictMode>
  );

  const { result } = renderHook(
    () => useStartupSync({ foreground, deferred: [] }),
    { wrapper },
  );
  await waitFor(() => expect(result.current.settled).toBe(true));
  expect(run).toHaveBeenCalledOnce();
});
