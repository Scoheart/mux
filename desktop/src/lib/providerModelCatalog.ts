import { discoverProviderModels } from "./api";
import type { ModelProviderInstanceView, ProviderModelSummary } from "./types";

// A presentation cache only: discovered models are never written to the library.
const MAX_AGE_MS = 5 * 60_000;
const catalogs = new Map<string, {
  key: string;
  at: number;
  request: Promise<ProviderModelSummary[]>;
}>();

export function providerCatalogKey(provider: ModelProviderInstanceView, revision: number): string {
  return JSON.stringify([
    provider.id, provider.provider, provider.base_url, provider.model_catalog_url,
    provider.protocols, provider.auth_requirement, provider.api_key_source,
    provider.credential_saved, revision,
  ]);
}

export function loadProviderModelCatalog(
  provider: ModelProviderInstanceView,
  revision: number,
  force = false,
): Promise<ProviderModelSummary[]> {
  const key = providerCatalogKey(provider, revision);
  const cached = catalogs.get(provider.id);
  if (!force && cached?.key === key && Date.now() - cached.at < MAX_AGE_MS) return cached.request;
  const request = discoverProviderModels(provider.id).catch((error: unknown) => {
    if (catalogs.get(provider.id)?.request === request) catalogs.delete(provider.id);
    throw error;
  });
  catalogs.set(provider.id, { key, at: Date.now(), request });
  return request;
}
