import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

interface TauriConfig {
  app?: {
    windows?: Array<{
      label?: string;
      title?: string;
      dragDropEnabled?: boolean;
    }>;
  };
}

const config = JSON.parse(
  await readFile(new URL("../../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
) as TauriConfig;

test("desktop windows leave HTML5 drag and drop enabled", () => {
  const windows = config.app?.windows ?? [];
  assert.ok(windows.length > 0, "expected at least one desktop window");

  for (const window of windows) {
    assert.equal(
      window.dragDropEnabled,
      false,
      `${window.label ?? window.title ?? "desktop window"} must disable Tauri's native drag-drop handler`,
    );
  }
});
