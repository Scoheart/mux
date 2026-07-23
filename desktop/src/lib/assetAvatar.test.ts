import { expect, it } from "vitest";
import { assetAvatarColor, assetAvatarIndex } from "./assetAvatar";

it("maps normalized asset names to stable palette colors", () => {
  expect(assetAvatarIndex("OpenRouter")).toBe(assetAvatarIndex(" openrouter "));
  expect(assetAvatarColor("OpenRouter")).toBe(assetAvatarColor(" openrouter "));
  expect(assetAvatarColor("OpenRouter")).not.toBe(assetAvatarColor("Anthropic"));
  expect(assetAvatarColor("OpenRouter")).toMatch(/^#[0-9A-F]{6}$/);
});
