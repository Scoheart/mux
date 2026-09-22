import type { AssetRef, ConsumptionInventory, ConsumptionView } from "./types";

export function assetIdentity(asset: AssetRef): string {
  if (asset.domain === "mcp") return asset.key;
  if (asset.domain === "model") return asset.profile_id;
  if (asset.domain === "model-provider") return asset.provider_id;
  return asset.name;
}

export function consumptionsForAgent(
  inventory: ConsumptionInventory | null,
  agentId: string,
  domain?: AssetRef["domain"],
): ConsumptionView[] {
  return stable(
    (inventory?.consumptions ?? []).filter(
      (item) =>
        item.agent_id === agentId &&
        item.desired &&
        (domain === undefined || item.asset.domain === domain),
    ),
  );
}

export function externalForAgent(
  inventory: ConsumptionInventory | null,
  agentId: string,
  domain?: AssetRef["domain"],
): ConsumptionView[] {
  return stable(
    (inventory?.external ?? []).filter(
      (item) =>
        item.agent_id === agentId &&
        (domain === undefined || item.asset.domain === domain),
    ),
  );
}

export function consumersForAsset(
  inventory: ConsumptionInventory | null,
  asset: AssetRef,
): ConsumptionView[] {
  const identity = assetIdentity(asset);
  return stable(
    (inventory?.consumptions ?? []).filter(
      (item) =>
        item.asset.domain === asset.domain &&
        assetIdentity(item.asset) === identity &&
        item.desired,
    ),
  );
}

export function observedAgentIdsForAsset(
  inventory: ConsumptionInventory | null,
  asset: AssetRef,
): string[] {
  const identity = assetIdentity(asset);
  return [...new Set([
    ...(inventory?.consumptions ?? []),
    ...(inventory?.external ?? []),
  ]
    .filter((item) =>
      item.asset.domain === asset.domain
      && assetIdentity(item.asset) === identity
      && item.observed
      && item.enabled !== false
    )
    .map((item) => item.agent_id))]
    .sort((left, right) => left.localeCompare(right));
}

/** Build once per inventory instead of scanning every relationship per card. */
export function observedAgentIndex(inventory: ConsumptionInventory | null, domain: AssetRef["domain"]): Map<string, string[]> {
  const byAsset = new Map<string, Set<string>>();
  for (const rows of [inventory?.consumptions ?? [], inventory?.external ?? []]) {
    for (const item of rows) {
      if (item.asset.domain !== domain || !item.observed || item.enabled === false) continue;
      const key = assetIdentity(item.asset);
      let agents = byAsset.get(key);
      if (!agents) byAsset.set(key, agents = new Set());
      agents.add(item.agent_id);
    }
  }
  return new Map([...byAsset].map(([key, agents]) => [
    key, [...agents].sort((left, right) => left.localeCompare(right)),
  ]));
}

function stable(items: ConsumptionView[]): ConsumptionView[] {
  return [...items].sort(
    (left, right) =>
      left.agent_id.localeCompare(right.agent_id) ||
      assetIdentity(left.asset).localeCompare(assetIdentity(right.asset)),
  );
}
