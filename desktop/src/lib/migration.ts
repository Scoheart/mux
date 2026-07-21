import type {
  McpAdoptionCandidate,
  ModelAdoptionCandidate,
  SkillInventoryItem,
} from "./types";

export type MigrationDomain = "mcp" | "model" | "skill";

export interface MigrationCandidate {
  id: string;
  domain: MigrationDomain;
  name: string;
  detail: string;
  agentIds: string[];
  fingerprint: string;
  safe: boolean;
  conflictReason: string | null;
  mcp?: {
    assetKey: string;
    candidateFingerprints: Record<string, string>;
  };
  skill?: {
    identity: string;
  };
  model?: {
    candidateFingerprints: Record<string, string>;
    provider: string;
    model: string;
    active: boolean;
  };
}

const blockedSkillStates = new Set([
  "locally_modified",
  "broken_link",
  "conflicting_link",
  "missing",
]);

export function buildMigrationCandidates(
  mcps: McpAdoptionCandidate[],
  skills: SkillInventoryItem[] | null,
  models: ModelAdoptionCandidate[] = [],
): MigrationCandidate[] {
  const candidates = [
    ...groupMcps(mcps),
    ...groupModels(models),
    ...groupSkills(skills),
  ];
  return candidates.sort((left, right) =>
    left.domain.localeCompare(right.domain) || left.name.localeCompare(right.name),
  );
}

function groupModels(items: ModelAdoptionCandidate[]): MigrationCandidate[] {
  const groups = new Map<string, ModelAdoptionCandidate[]>();
  for (const item of items) {
    const rows = groups.get(item.fingerprint) ?? [];
    rows.push(item);
    groups.set(item.fingerprint, rows);
  }
  return [...groups.entries()].map(([fingerprint, rows]) => {
    rows.sort((left, right) => Number(right.active) - Number(left.active)
      || left.agent_id.localeCompare(right.agent_id));
    const uniqueAgents = new Set(rows.map((row) => row.agent_id)).size === rows.length;
    const safe = uniqueAgents && rows.every((row) => row.status === "adoptable");
    const primary = rows[0];
    const activeCount = rows.filter((row) => row.active).length;
    const conflictReason = !uniqueAgents
      ? "同一 Agent 中多个模型共用 native provider identity；请先拆分 provider 后再导入"
      : safe
      ? null
      : rows.find((row) => row.status !== "adoptable")?.reason
        ?? "该模型需要先处理 credential 或配置冲突";
    return {
      id: `model:${fingerprint}`,
      domain: "model",
      name: primary.name || primary.model,
      detail: `${primary.provider} · ${primary.model} · ${rows.length} 个 Agent${activeCount ? ` · ${activeCount} 个当前使用` : ""}`,
      agentIds: rows.map((row) => row.agent_id),
      fingerprint: `model:${fingerprint}:${rows.map((row) => row.candidate_hash).join(":")}`,
      safe,
      conflictReason,
      model: {
        candidateFingerprints: Object.fromEntries(rows.map((row) => [row.candidate_id, row.fingerprint])),
        provider: primary.provider,
        model: primary.model,
        active: activeCount > 0,
      },
    };
  });
}

function groupMcps(items: McpAdoptionCandidate[]): MigrationCandidate[] {
  const groups = new Map<string, McpAdoptionCandidate[]>();
  for (const item of items) {
    const rows = groups.get(item.asset_key) ?? [];
    rows.push(item);
    groups.set(item.asset_key, rows);
  }
  return [...groups.entries()].map(([assetKey, rows]) => {
    rows.sort((left, right) => left.agent_id.localeCompare(right.agent_id));
    const hashes = new Set(rows.map((row) => row.config_hash));
    const statuses = new Set(rows.map((row) => row.status));
    const drifted = rows.some((row) => row.status === "drifted");
    const safe = hashes.size === 1 && statuses.size === 1 && !drifted;
    const [name, transport] = splitAssetKey(assetKey);
    const disabled = rows.filter((row) => !row.enabled).length;
    const centralExists = rows.every((row) => row.status === "adoptable");
    const conflictReason = safe
      ? null
      : drifted
        ? "外部配置与中央资产不一致；请先在 MUX 或原 Agent 中统一后重新扫描"
        : "同名 MCP 的连接配置不一致；请先在原 Agent 中统一或重命名后重新扫描";
    return {
      id: `mcp:${assetKey}`,
      domain: "mcp",
      name,
      detail: `${transport.toUpperCase()} · ${rows.length} 个 Agent${disabled > 0 ? ` · ${disabled} 个停用` : ""}${centralExists ? " · 原地认领" : " · 创建中央副本"}`,
      agentIds: rows.map((row) => row.agent_id),
      fingerprint: `mcp:${assetKey}:${rows.map((row) => row.fingerprint).join(":")}`,
      safe,
      conflictReason,
      mcp: {
        assetKey,
        candidateFingerprints: Object.fromEntries(
          rows.map((row) => [row.agent_id, row.fingerprint]),
        ),
      },
    };
  });
}

function groupSkills(items: SkillInventoryItem[] | null): MigrationCandidate[] {
  if (!items) return [];
  const centralNames = new Set(
    items
      .filter((item) => item.location.kind === "central")
      .map((item) => item.name),
  );
  const groups = new Map<string, SkillInventoryItem[]>();
  for (const item of items) {
    if (
      item.location.kind !== "agent_target" ||
      !item.states.includes("external")
    ) {
      continue;
    }
    const rows = groups.get(item.name) ?? [];
    rows.push(item);
    groups.set(item.name, rows);
  }
  return [...groups.entries()].map(([name, rows]) => {
    rows.sort((left, right) => left.identity.localeCompare(right.identity));
    const hashes = new Set(rows.map((row) => row.content_hash).filter(Boolean));
    const agentIds = [...new Set(rows.flatMap((row) => row.affected_agent_ids))].sort();
    const invalid = agentIds.length === 0 || rows.some(
      (row) =>
        !row.content_hash ||
        row.states.some((state) => blockedSkillStates.has(state)),
    );
    const centralConflict = centralNames.has(name);
    const highRisk = rows.some((row) => row.risk?.level === "high");
    const missingAudit = rows.some((row) => row.risk === null);
    const safe = !invalid && !centralConflict && !highRisk && !missingAudit && hashes.size === 1;
    let conflictReason: string | null = null;
    if (centralConflict) {
      conflictReason = "中央资产库已存在同名 Skill；请先重命名来源目录或处理中央冲突后重新扫描";
    } else if (highRisk) {
      conflictReason = "Skill 包含高风险内容；请在 Skills 页面单独导入并审阅风险";
    } else if (missingAudit) {
      conflictReason = "Skill 风险检查未完成；请修复内容后重新扫描";
    } else if (hashes.size > 1) {
      conflictReason = "同名 Skill 的内容不一致；请先统一内容或重命名来源目录后重新扫描";
    } else if (!safe) {
      conflictReason = "Skill 目录损坏或无法安全读取；请修复后重新扫描";
    }
    const hash = hashes.values().next().value ?? "unavailable";
    return {
      id: `skill:${name}`,
      domain: "skill",
      name,
      detail: `${agentIds.length} 个 Agent · ${rows.length} 个目录 · 合并为一份中央副本`,
      agentIds,
      fingerprint: `skill:${name}:${hash}:${rows.map((row) => row.identity).join(":")}`,
      safe,
      conflictReason,
      skill: { identity: rows[0].identity },
    };
  });
}

function splitAssetKey(key: string): [string, string] {
  const index = key.lastIndexOf("::");
  return index < 0 ? [key, "mcp"] : [key.slice(0, index), key.slice(index + 2)];
}

export const migrationCounts = (items: MigrationCandidate[]) => ({
  all: items.length,
  mcp: items.filter((item) => item.domain === "mcp").length,
  model: items.filter((item) => item.domain === "model").length,
  skill: items.filter((item) => item.domain === "skill").length,
  safe: items.filter((item) => item.safe).length,
  conflicts: items.filter((item) => !item.safe).length,
});
