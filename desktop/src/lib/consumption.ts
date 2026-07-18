import type { AssetRef, ConsumptionInventory, ConsumptionView } from "./types";

export function assetIdentity(asset: AssetRef): string {
  if (asset.domain === "mcp") return asset.key;
  if (asset.domain === "model") return asset.profile_id;
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

function stable(items: ConsumptionView[]): ConsumptionView[] {
  return [...items].sort(
    (left, right) =>
      left.agent_id.localeCompare(right.agent_id) ||
      assetIdentity(left.asset).localeCompare(assetIdentity(right.asset)),
  );
}
