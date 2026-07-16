import { readFile } from "node:fs/promises";
import { expect, it } from "vitest";

interface TauriConfig {
  app?: {
    windows?: Array<{
      label?: string;
      title?: string;
      dragDropEnabled?: boolean;
    }>;
  };
}

const relativeFile = (path: string) => new URL(path, import.meta.url);

const config = JSON.parse(
  await readFile(relativeFile("../../src-tauri/tauri.conf.json"), "utf8"),
) as TauriConfig;

it("desktop windows leave HTML5 drag and drop enabled", () => {
  const windows = config.app?.windows ?? [];
  expect(windows.length, "expected at least one desktop window").toBeGreaterThan(0);

  for (const window of windows) {
    expect(
      window.dragDropEnabled,
      `${window.label ?? window.title ?? "desktop window"} must disable Tauri's native drag-drop handler`,
    ).toBe(false);
  }
});
