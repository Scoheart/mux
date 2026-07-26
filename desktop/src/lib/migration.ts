import type {
  McpAdoptionCandidate,
  ModelAdoptionCandidate,
  SkillInventoryItem,
} from "./types";

export type MigrationDomain = "mcp" | "model" | "skill";

export type MigrationCandidateDetail =
  | {
    kind: "model";
    provider: string;
    model: string;
    agentCount: number;
    activeCount: number;
  }
  | {
    kind: "mcp";
    transport: string;
    agentCount: number;
    disabledCount: number;
    centralExists: boolean;
  }
  | {
    kind: "skill";
    agentCount: number;
    folderCount: number;
  };

export type MigrationConflict =
  | { kind: "model_shared_provider_identity" }
  | { kind: "model_multiple_credentials" }
  | { kind: "model_external_credential_command" }
  | { kind: "model_environment_to_keychain" }
  | { kind: "model_literal_to_environment" }
  | { kind: "model_unsafe_native_identity" }
  | { kind: "model_shared_native_provider" }
  | { kind: "model_source"; reason: string }
  | { kind: "model_credential_or_config" }
  | { kind: "mcp_drifted" }
  | { kind: "mcp_connection_mismatch" }
  | { kind: "skill_central_conflict" }
  | { kind: "skill_high_risk" }
  | { kind: "skill_missing_audit" }
  | { kind: "skill_content_mismatch" }
  | { kind: "skill_invalid" };

export function mcpMigrationCandidateId(assetKey: string) {
  return `mcp:${assetKey}`;
}

export function modelMigrationCandidateId(fingerprint: string) {
  return `model:${fingerprint}`;
}

export function skillMigrationCandidateId(name: string) {
  return `skill:${name}`;
}

export interface MigrationCandidate {
  id: string;
  domain: MigrationDomain;
  name: string;
  detail: MigrationCandidateDetail;
  agentIds: string[];
  fingerprint: string;
  safe: boolean;
  conflict: MigrationConflict | null;
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
    const conflict: MigrationConflict | null = !uniqueAgents
      ? { kind: "model_shared_provider_identity" }
      : safe
      ? null
      : rows.find((row) => row.status !== "adoptable")?.reason
        ? modelConflictFromReason(
          rows.find((row) => row.status !== "adoptable")!.reason!,
        )
        : { kind: "model_credential_or_config" };
    return {
      id: modelMigrationCandidateId(fingerprint),
      domain: "model",
      name: primary.name || primary.model,
      detail: {
        kind: "model",
        provider: primary.provider,
        model: primary.model,
        agentCount: rows.length,
        activeCount,
      },
      agentIds: rows.map((row) => row.agent_id),
      fingerprint: `model:${fingerprint}:${rows.map((row) => row.candidate_hash).join(":")}`,
      safe,
      conflict,
      model: {
        candidateFingerprints: Object.fromEntries(rows.map((row) => [row.candidate_id, row.fingerprint])),
        provider: primary.provider,
        model: primary.model,
        active: activeCount > 0,
      },
    };
  });
}

function modelConflictFromReason(reason: string): MigrationConflict {
  switch (reason) {
    case "检测到多个不同的明文 credential，不能安全合并":
      return { kind: "model_multiple_credentials" };
    case "外部 credential command 不会被执行；请先改为明文一次性导入或安全环境变量":
      return { kind: "model_external_credential_command" };
    case "该 Agent 的 MUX writer 使用 Keychain，不能无损接管外部环境变量引用":
      return { kind: "model_environment_to_keychain" };
    case "该 Agent 仅支持环境变量引用；请先把明文 Key 改为环境变量":
      return { kind: "model_literal_to_environment" };
    case "Agent-native provider identity 含有不安全字符，MUX 不会把它用于配置键或文件名":
      return { kind: "model_unsafe_native_identity" };
    case "多个 Model 共用同一个 Agent-native provider；请先在 Agent 中拆分 provider identity，MUX 不会覆盖兄弟模型":
      return { kind: "model_shared_native_provider" };
    default:
      return { kind: "model_source", reason };
  }
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
    const conflict: MigrationConflict | null = safe
      ? null
      : drifted
        ? { kind: "mcp_drifted" }
        : { kind: "mcp_connection_mismatch" };
    return {
      id: mcpMigrationCandidateId(assetKey),
      domain: "mcp",
      name,
      detail: {
        kind: "mcp",
        transport: transport.toUpperCase(),
        agentCount: rows.length,
        disabledCount: disabled,
        centralExists,
      },
      agentIds: rows.map((row) => row.agent_id),
      fingerprint: `mcp:${assetKey}:${rows.map((row) => row.fingerprint).join(":")}`,
      safe,
      conflict,
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
    let conflict: MigrationConflict | null = null;
    if (centralConflict) {
      conflict = { kind: "skill_central_conflict" };
    } else if (highRisk) {
      conflict = { kind: "skill_high_risk" };
    } else if (missingAudit) {
      conflict = { kind: "skill_missing_audit" };
    } else if (hashes.size > 1) {
      conflict = { kind: "skill_content_mismatch" };
    } else if (!safe) {
      conflict = { kind: "skill_invalid" };
    }
    const hash = hashes.values().next().value ?? "unavailable";
    return {
      id: skillMigrationCandidateId(name),
      domain: "skill",
      name,
      detail: {
        kind: "skill",
        agentCount: agentIds.length,
        folderCount: rows.length,
      },
      agentIds,
      fingerprint: `skill:${name}:${hash}:${rows.map((row) => row.identity).join(":")}`,
      safe,
      conflict,
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
