import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { listModelProfiles, revealModelProviderCredential } from "./api";

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

  it("uses a dedicated command to reveal one Provider credential", async () => {
    invokeMock.mockResolvedValueOnce("saved-test-value");

    await expect(revealModelProviderCredential("team-provider"))
      .resolves.toBe("saved-test-value");
    expect(invokeMock).toHaveBeenCalledWith(
      "reveal_model_provider_credential",
      { providerId: "team-provider" },
    );
  });
});
