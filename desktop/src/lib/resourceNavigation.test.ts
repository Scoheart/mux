import { describe, expect, it } from "vitest";
import {
  clearResourceIntent,
  createResourceNavigationIntent,
  viewForResourceIntent,
  viewHasResourceIntent,
} from "./resourceNavigation";

describe("resource navigation", () => {
  it("routes every domain to its top-level view", () => {
    const mcp = createResourceNavigationIntent(1, { domain: "mcp", kind: "detail", name: "fs", transport: "stdio" });
    const model = createResourceNavigationIntent(2, { domain: "model", kind: "detail", profileId: "gateway" });
    const skill = createResourceNavigationIntent(3, { domain: "skill", kind: "install", agentId: "codex" });
    expect(viewForResourceIntent(mcp)).toEqual({ kind: "registry", intent: mcp });
    expect(viewForResourceIntent(model)).toEqual({ kind: "models", intent: model });
    expect(viewForResourceIntent(skill)).toEqual({ kind: "skills", intent: skill });
  });

  it("consumes only the matching intent once", () => {
    const intent = createResourceNavigationIntent(7, { domain: "model", kind: "detail", profileId: "gateway" });
    const view = viewForResourceIntent(intent);
    expect(viewHasResourceIntent(view, 7)).toBe(true);
    expect(clearResourceIntent(view, 6)).toBe(view);
    expect(clearResourceIntent(view, 7)).toEqual({ kind: "models" });
  });
});
