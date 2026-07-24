import { expect, it, vi } from "vitest";
import type { ModelProfileView } from "./types";
import {
  getCachedModelsDevMetadata,
  loadModelsDevMetadata,
  MODELS_DEV_API_URL,
  MODELS_DEV_CACHE_TTL_MS,
} from "./modelsDev";

function profile(overrides: Partial<ModelProfileView> = {}): ModelProfileView {
  return {
    id: "openrouter-profile",
    name: "OpenRouter",
    provider: "openrouter",
    protocol: "openai-completions",
    base_url: "https://openrouter.ai/api/v1",
    model: "qwen/qwen3",
    reasoning: false,
    catalog_key: "openrouter/qwen/qwen3",
    credential_saved: true,
    ...overrides,
  };
}

function memoryStorage(initial?: string) {
  let value = initial ?? null;
  return {
    getItem: vi.fn(() => value),
    setItem: vi.fn((_key: string, next: string) => { value = next; }),
  };
}

const catalog = {
  openrouter: {
    models: {
      "qwen/qwen3": {
        id: "qwen/qwen3",
        name: "Qwen3",
        description: "A useful reasoning model.",
        reasoning: true,
        tool_call: true,
        structured_output: true,
        modalities: { input: ["text", "image"], output: ["text"] },
        release_date: "2026-01-02",
        limit: { context: 262_144, output: 32_768 },
        cost: { input: 0.2, output: 0.8 },
      },
    },
  },
};

it("matches OpenRouter profiles and stores only compact requested metadata", async () => {
  const storage = memoryStorage();
  const fetchImpl = vi.fn(async () => new Response(JSON.stringify(catalog)));

  const result = await loadModelsDevMetadata([profile()], {
    fetchImpl,
    storage,
    now: () => 1_000,
  });

  expect(fetchImpl).toHaveBeenCalledWith(
    MODELS_DEV_API_URL,
    expect.objectContaining({ signal: expect.any(AbortSignal) }),
  );
  expect(result["openrouter-profile"]).toEqual({
    name: "Qwen3",
    description: "A useful reasoning model.",
    reasoning: true,
    toolCall: true,
    structuredOutput: true,
    modalities: ["text", "image"],
    releaseDate: "2026-01-02",
    contextWindow: 262_144,
    maxOutputTokens: 32_768,
    inputCost: 0.2,
    outputCost: 0.8,
  });
  const stored = JSON.parse(storage.setItem.mock.calls[0][1]);
  expect(JSON.stringify(stored)).not.toContain("openrouter\":{\"models\"");
  expect(Object.keys(stored.entries)).toEqual(["openrouter::qwen/qwen3"]);
});

it("uses a fresh subset cache without a network request", async () => {
  const storage = memoryStorage(JSON.stringify({
    version: 1,
    fetchedAt: 2_000,
    entries: {
      "openrouter::qwen/qwen3": { name: "Cached Qwen", contextWindow: 128_000 },
    },
  }));
  const fetchImpl = vi.fn();

  const result = await loadModelsDevMetadata([profile()], {
    fetchImpl,
    storage,
    now: () => 2_000 + MODELS_DEV_CACHE_TTL_MS - 1,
  });

  expect(result["openrouter-profile"]?.name).toBe("Cached Qwen");
  expect(fetchImpl).not.toHaveBeenCalled();
});

it("falls back to cached metadata when refresh fails and never guesses Custom models", async () => {
  const storage = memoryStorage(JSON.stringify({
    version: 1,
    fetchedAt: 1,
    entries: {
      "openrouter::qwen/qwen3": { name: "Offline Qwen", inputCost: 0.2 },
    },
  }));
  const fetchImpl = vi.fn(async () => { throw new Error("offline"); });
  const custom = profile({
    id: "custom",
    provider: "custom",
    base_url: "https://gateway.example.test/v1",
    model: "private-model",
    catalog_key: "custom/private-model",
  });

  const result = await loadModelsDevMetadata([profile(), custom], {
    fetchImpl,
    storage,
    now: () => MODELS_DEV_CACHE_TTL_MS + 2,
  });

  expect(result["openrouter-profile"]?.name).toBe("Offline Qwen");
  expect(result.custom).toBeUndefined();
  expect(getCachedModelsDevMetadata([profile()], storage)["openrouter-profile"]?.name)
    .toBe("Offline Qwen");
});
