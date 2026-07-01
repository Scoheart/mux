import type { ActiveMcp, ScannedMcp, DiffEntry } from "../types.js";

export function computeDiff(
  desired: ActiveMcp[],
  current: ScannedMcp[]
): DiffEntry[] {
  const diffs: DiffEntry[] = [];

  const desiredSet = new Set<string>();
  for (const mcp of desired) {
    const scopes: Array<"global" | "project"> =
      mcp.scope === "both" ? ["global", "project"] : [mcp.scope];

    for (const scope of scopes) {
      for (const agent of mcp.agents) {
        const key = `${mcp.name}|${agent}|${scope}`;
        desiredSet.add(key);

        const exists = current.some(
          (c) => c.name === mcp.name && c.source.agent === agent && c.source.scope === scope
        );

        if (!exists) {
          diffs.push({ action: "add", mcpName: mcp.name, agent, scope });
        }
      }
    }
  }

  for (const c of current) {
    const key = `${c.name}|${c.source.agent}|${c.source.scope}`;
    if (!desiredSet.has(key)) {
      diffs.push({
        action: "remove",
        mcpName: c.name,
        agent: c.source.agent,
        scope: c.source.scope,
      });
    }
  }

  return diffs;
}
