import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { UpdaterState } from "../hooks/useUpdater";
import { UpdateBanner } from "./UpdateBanner";

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

afterEach(cleanup);

function updater(percent: number | null): UpdaterState {
  return {
    phase: { kind: "downloading", percent },
    checkNow: vi.fn(),
    download: vi.fn(),
    restart: vi.fn(),
    dismiss: vi.fn(),
    later: vi.fn(),
  };
}

it("keeps the determinate fill synchronized with the displayed percentage", () => {
  render(<UpdateBanner updater={updater(37)} />);

  expect(screen.getByText("正在下载更新 37%")).toBeVisible();
  const progress = screen.getByRole("progressbar", { name: "更新下载进度" });
  expect(progress).toHaveAttribute("aria-valuenow", "37");
  expect(progress.firstElementChild).toHaveStyle({ width: "37%" });
  expect((progress.firstElementChild as HTMLElement).style.transition).toBe("");
});

it("keeps unknown-length downloads indeterminate", () => {
  render(<UpdateBanner updater={updater(null)} />);

  expect(screen.getByText("正在下载更新…")).toBeVisible();
  const progress = screen.getByRole("progressbar", { name: "更新下载进度" });
  expect(progress).not.toHaveAttribute("aria-valuenow");
});
