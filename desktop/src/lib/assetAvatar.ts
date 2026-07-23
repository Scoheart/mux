const ASSET_AVATAR_PALETTE = [
  "#2563EB",
  "#7C3AED",
  "#C2410C",
  "#047857",
  "#B91C1C",
  "#0E7490",
  "#A21CAF",
  "#4D7C0F",
  "#4338CA",
  "#BE123C",
  "#0369A1",
  "#6D28D9",
] as const;

export function assetAvatarIndex(seed: string) {
  const normalized = seed.normalize("NFKC").trim().toLocaleLowerCase();
  let hash = 2166136261;
  for (const character of normalized) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619) >>> 0;
  }
  return hash % ASSET_AVATAR_PALETTE.length;
}

export function assetAvatarColor(seed: string) {
  return ASSET_AVATAR_PALETTE[assetAvatarIndex(seed)];
}
