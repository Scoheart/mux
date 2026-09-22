import { listModelProfiles, listModelProviderInstances, listModelProviders } from "./api";
import { modelObservationRevision } from "./modelObservation";
import type { ModelProfileView, ModelProviderInstanceView, ModelProviderView } from "./types";

interface ModelLibrary {
  profiles: ModelProfileView[];
  providers: ModelProviderView[];
  instances: ModelProviderInstanceView[];
}

// Presentation-only cache. No credential values; mutations and cURL copies
// always call core. Watcher/rescan invalidation takes effect immediately.
const MAX_AGE_MS = 30_000;
let cached: { revision: number; at: number; value: ModelLibrary } | null = null;
let pending: { revision: number; request: Promise<ModelLibrary> } | null = null;

export function cachedModelLibrary(): ModelLibrary | null {
  return cached && cached.revision === modelObservationRevision() && Date.now() - cached.at < MAX_AGE_MS
    ? cached.value : null;
}

export function loadModelLibrary(force = false): Promise<ModelLibrary> {
  if (force) cached = null;
  const revision = modelObservationRevision();
  const value = !force && cachedModelLibrary();
  if (value) return Promise.resolve(value);
  if (!force && pending?.revision === revision) return pending.request;
  const request = Promise.all([listModelProfiles(), listModelProviders(), listModelProviderInstances()])
    .then(([profiles, providers, instances]) => {
      const value = { profiles, providers, instances };
      if (pending?.request === request && revision === modelObservationRevision()) {
        cached = { revision, at: Date.now(), value };
      }
      return value;
    }).finally(() => {
      if (pending?.request === request) pending = null;
    });
  pending = { revision, request };
  return request;
}
