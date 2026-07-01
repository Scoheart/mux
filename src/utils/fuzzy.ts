import Fuse from "fuse.js";
import type { RegistryEntry } from "../types.js";

export function createMcpSearcher(entries: RegistryEntry[]) {
  const fuse = new Fuse(entries, {
    keys: [
      { name: "name", weight: 3 },
      { name: "description", weight: 2 },
      { name: "tags", weight: 1 },
    ],
    threshold: 0.4,
    includeScore: true,
  });

  return (query: string): RegistryEntry[] => {
    if (!query.trim()) return entries;
    return fuse.search(query).map((r) => r.item);
  };
}
