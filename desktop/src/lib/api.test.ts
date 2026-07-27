import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { listModelProfiles } from "./api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
});

describe("Model Profile wire contract", () => {
  it("restores empty strings omitted by compact Rust serialization", async () => {
    invokeMock.mockResolvedValueOnce([{
      id: "legacy-profile",
      name: "Legacy Profile",
      protocol: "anthropic-messages",
      model: "vendor/model",
      catalog_key: "/vendor/model",
      credential_saved: false,
    }]);

    await expect(listModelProfiles()).resolves.toEqual([{
      id: "legacy-profile",
      name: "Legacy Profile",
      provider: "",
      protocol: "anthropic-messages",
      base_url: "",
      model: "vendor/model",
      catalog_key: "/vendor/model",
      credential_saved: false,
    }]);
    expect(invokeMock).toHaveBeenCalledWith("list_model_profiles");
  });
});
