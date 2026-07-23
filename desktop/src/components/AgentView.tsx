import { useCallback, useEffect, useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { InstallState } from "../hooks/useInstallState";
import type { SkillsState } from "../hooks/useSkillsState";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import type {
  AgentConsumptionSelection,
  AssetOperationPlan,
  AssetRef,
  ConsumptionInventory,
  ModelAdoptionCandidate,
  ModelAgentView,
  ModelProfileView,
  ResourceNavigationRequest,
} from "../lib/types";
import { formatError } from "../lib/format";
import { keyOf, transportOf } from "../lib/mcp";
import { consumptionsForAgent, externalForAgent } from "../lib/consumption";
import { listModelAgents, listModelProfiles } from "../lib/api";
import { EditIcon, LinkIcon, PlusIcon, SparklesIcon } from "./icons";
import { Avatar, Badge, IconButton } from "./ui";
import { AgentGlyph } from "./brandIcons";
import { AgentConfigurationDialog } from "./AgentConfigurationDialog";
import { useToast } from "./Toast";
import { AgentResourcePanel, type AgentResourceTab } from "./AgentResourcePanel";
import { AgentConsumptionPanel } from "./AgentConsumptionPanel";
import {
  ConsumptionPickerDialog,
  type ConsumptionPickerOption,
} from "./ConsumptionPickerDialog";
import { AssetOperationReviewDialog } from "./AssetOperationReviewDialog";
import { modelMigrationCandidateId, skillMigrationCandidateId } from "../lib/migration";

type PickerDomain = "mcp" | "model" | "skill";

function modelProtocolLabel(protocol: ModelProfileView["protocol"]) {
  if (protocol === "anthropic-messages") return "Anthropic Messages";
  if (protocol === "openai-responses") return "OpenAI Responses";
  return "OpenAI Chat Completions";
}

function modelCompatibilityReason(profile: ModelProfileView, agent: ModelAgentView | null) {
  if (!agent || agent.mode !== "managed") return "此 Agent 不支持 MUX Model 管理";
  if (!agent.supported_protocols.includes(profile.protocol)) return "协议不兼容";
  if (agent.credential_mode === "environment-reference" && profile.credential_saved && !profile.env_key) {
    return "此 Agent 需要 Profile 提供环境变量名";
  }
  return null;
}

interface AgentViewProps {
  state: InstallState;
  skillsState: SkillsState;
  consumptionState?: ConsumptionState;
  agentId: string;
  modelMigrationCandidates?: ModelAdoptionCandidate[];
  onOpenResource?(request: ResourceNavigationRequest): void;
  /** Transitional test adapters. */
  onOpenModels?: () => void;
  onOpenSkills?: (request: Extract<ResourceNavigationRequest, { domain: "skill" }>) => void;
  onOpenMigration?: (focusId?: string | null) => void;
  onManageExternalMcp?: (assetKey: string) => void;
}

function fallbackInventory(
  state: InstallState,
  modelAgents: ModelAgentView[],
): ConsumptionInventory {
  return {
    consumptions: [
      ...state.installed
        .filter((item) => item.scope === "global")
        .map((item) => ({
          agent_id: item.agent,
          asset: { domain: "mcp" as const, key: `${item.name}::${item.transport}` },
          desired: true,
          observed: true,
          enabled: item.enabled,
          status: item.customized ? "drifted" as const : "synced" as const,
          reason: item.customized ? "mcp_config_drift" : null,
          affected_agent_ids: [item.agent],
        })),
      ...modelAgents.flatMap((item) =>
        (item.assigned_profiles ?? (item.assigned_profile ? [item.assigned_profile] : [])).map((profileId) => ({
              agent_id: item.id,
              asset: { domain: "model" as const, profile_id: profileId },
              desired: true,
              observed: true,
              enabled: true,
              active: profileId === (item.active_profile ?? item.assigned_profile),
              status: "synced" as const,
              reason: null,
              affected_agent_ids: [item.id],
            })),
      ),
    ],
    external: [],
  };
}

function completedMessage(plan: AssetOperationPlan, agentName: string) {
  const domain = plan.domain_plan.domain;
  const asset = domain === "mcp" ? "MCP" : domain === "model" ? "Model" : "Skill";
  const hasAdd = plan.relationship_changes.some((change) => change.action === "add");
  const hasRemove = plan.relationship_changes.some((change) => change.action === "remove");
  if (domain === "model" && hasAdd) return `Model 已添加到 ${agentName}。`;
  if (hasAdd && !hasRemove) return `${asset} 已添加到 ${agentName}。`;
  if (hasRemove && !hasAdd) return `${asset} 已从 ${agentName} 移除。`;
  if (domain === "model") {
    if (plan.model_state_changes.some((change) => change.reason === "model_disabled")) {
      return `${agentName} 的 Model 已停用。`;
    }
    if (plan.model_state_changes.some((change) => change.reason === "model_enabled")) {
      return `${agentName} 的 Model 已启用。`;
    }
    return `${agentName} 的当前 Model 已更新。`;
  }
  return `${agentName} 的 ${asset} 已更新。`;
}

function requiresAgentReview(plan: AssetOperationPlan) {
  return !plan.can_commit
    || plan.requires_conflict_confirmation
    || plan.warnings.length > 0
    || plan.affected_agent_ids.length > 1
    || plan.model_state_changes.some((change) =>
      change.before.active && !change.after.active && (!change.after.enabled || !change.after.added)
    );
}

export function AgentView({
  state,
  skillsState,
  consumptionState,
  agentId,
  modelMigrationCandidates = [],
  onOpenResource,
  onOpenModels,
  onOpenSkills,
  onOpenMigration,
  onManageExternalMcp,
}: AgentViewProps) {
  const { entries, agents, refreshAgents, rescan } = state;
  const { show: showToast } = useToast();
  const [editingAgent, setEditingAgent] = useState(false);
  const [pickerDomain, setPickerDomain] = useState<PickerDomain | null>(null);
  const [modelProfiles, setModelProfiles] = useState<ModelProfileView[]>([]);
  const [modelAgents, setModelAgents] = useState<ModelAgentView[]>([]);
  const [modelsLoading, setModelsLoading] = useState(true);
  const [modelsError, setModelsError] = useState<string | null>(null);
  const [resourceTab, setResourceTab] = useState<AgentResourceTab>("mcps");
  const [preparingChange, setPreparingChange] = useState(false);
  const [togglingMcp, setTogglingMcp] = useState<{
    key: string;
    enabled: boolean;
  } | null>(null);
  const [changingModel, setChangingModel] = useState<{
    profileId: string;
    kind: "enabled" | "active";
    enabled?: boolean;
  } | null>(null);

  const navigateResource = useCallback((request: ResourceNavigationRequest) => {
    if (onOpenResource) return onOpenResource(request);
    if (request.domain === "skill") return onOpenSkills?.(request);
    if (request.domain === "model") return onOpenModels?.();
  }, [onOpenModels, onOpenResource, onOpenSkills]);

  const agent = useMemo(
    () => agents.find((item) => item.id === agentId) ?? null,
    [agents, agentId],
  );

  useEffect(() => {
    if (agent && !agent.has_global && agent.skills_global_dir) {
      setResourceTab("skills");
    }
  }, [agent?.has_global, agent?.id, agent?.skills_global_dir]);

  const refreshModels = useCallback(async () => {
    try {
      const [profiles, nextAgents] = await Promise.all([listModelProfiles(), listModelAgents()]);
      setModelProfiles(profiles);
      setModelAgents(nextAgents);
      setModelsError(null);
    } catch (error) {
      setModelsError(formatError(error));
      throw error;
    }
  }, []);

  useEffect(() => {
    setModelsLoading(true);
    setModelsError(null);
    refreshModels()
      .catch((error) => showToast({ kind: "error", msg: "读取模型配置失败：" + formatError(error) }))
      .finally(() => setModelsLoading(false));
  }, [refreshModels, showToast]);

  const modelAgent = useMemo(
    () => modelAgents.find((item) => item.id === agentId) ?? null,
    [modelAgents, agentId],
  );
  const compatibleProfiles = useMemo(
    () => modelAgent
      ? modelProfiles.filter((profile) => modelCompatibilityReason(profile, modelAgent) === null)
      : [],
    [modelAgent, modelProfiles],
  );
  const inventory = consumptionState?.inventory ?? fallbackInventory(state, modelAgents);
  const mcpRows = consumptionsForAgent(inventory, agentId, "mcp");
  const modelRows = consumptionsForAgent(inventory, agentId, "model");
  const skillRows = consumptionsForAgent(inventory, agentId, "skill");
  const displayedMcpRows = useMemo(
    () => mcpRows.map((item) => (
      togglingMcp && item.asset.domain === "mcp" && item.asset.key === togglingMcp.key
        ? { ...item, enabled: togglingMcp.enabled }
        : item
    )),
    [mcpRows, togglingMcp],
  );
  const mcpExternal = externalForAgent(inventory, agentId, "mcp");
  const skillExternal = externalForAgent(inventory, agentId, "skill");
  const agentModelMigrationCandidates = useMemo(
    () => modelMigrationCandidates
      .filter((candidate) => candidate.agent_id === agentId)
      .sort((left, right) => Number(right.active) - Number(left.active)
        || (left.name || left.model).localeCompare(right.name || right.model)),
    [agentId, modelMigrationCandidates],
  );
  const externalModelCandidateByProfileId = useMemo(
    () => new Map(agentModelMigrationCandidates.map((candidate) => [
      `external-model:${candidate.candidate_id}`,
      candidate,
    ])),
    [agentModelMigrationCandidates],
  );
  const modelExternalCards = useMemo(
    () => agentModelMigrationCandidates.map((candidate) => ({
      agent_id: agentId,
      asset: { domain: "model" as const, profile_id: `external-model:${candidate.candidate_id}` },
      desired: false,
      observed: true,
      enabled: false,
      active: candidate.active,
      desired_active: false,
      status: "external" as const,
      reason: candidate.reason ?? `model_${candidate.status}`,
      affected_agent_ids: [agentId],
    })),
    [agentId, agentModelMigrationCandidates],
  );
  const displayedModelRows = useMemo(
    () => modelRows.map((item) => (
      changingModel?.kind === "enabled"
      && item.asset.domain === "model"
      && item.asset.profile_id === changingModel.profileId
        ? { ...item, enabled: changingModel.enabled }
        : item
    )),
    [changingModel, modelRows],
  );

  if (!agent) return <div className="mux-agent-state">未找到该 Agent</div>;

  if (!agent.has_global && !agent.skills_global_dir) {
    return (
      <div className="mux-agent-page">
        <div className="mux-agent-shell">
          <section className="mux-agent-context" aria-label={`${agent.name} 参考信息`}>
            <AgentHeader agent={agent} tone="reference" />
            <div className="mux-agent-reference">
              <strong>{agent.note ?? "未提供可写的用户级全局配置。"}</strong>
            </div>
          </section>
        </div>
      </div>
    );
  }

  const runtimeSkillAgent = skillsState.inventory?.agents.find((item) => item.id === agentId) ?? null;

  const centralSkills = (skillsState.inventory?.items ?? []).filter(
    (item) => item.location.kind === "central" && item.states.includes("managed"),
  );

  const currentIds = (domain: AssetRef["domain"]): string[] => {
    if (domain === "mcp") {
      return mcpRows.flatMap((item) => item.asset.domain === "mcp" ? [item.asset.key] : []);
    }
    if (domain === "model") {
      return modelRows.flatMap((item) => item.asset.domain === "model" ? [item.asset.profile_id] : []);
    }
    return skillRows.flatMap((item) => item.asset.domain === "skill" ? [item.asset.name] : []);
  };
  const picker = pickerDomain ? pickerData(pickerDomain) : null;

  function pickerData(domain: PickerDomain): {
    title: string;
    mode: "single" | "multiple";
    actionLabel: string;
    busyLabel: string;
    emptyMessage: string;
    searchPlaceholder: string;
    options: ConsumptionPickerOption[];
  } {
    const assigned = new Set(currentIds(domain));
    if (domain === "mcp") {
      return {
        title: "添加 MCP",
        mode: "multiple",
        actionLabel: "添加 MCP",
        busyLabel: "添加中…",
        emptyMessage: "没有可添加的 MCP",
        searchPlaceholder: "搜索 MCP",
        options: entries
          .filter((entry) => agent?.supported_transports.includes(transportOf(entry)) && !assigned.has(keyOf(entry)))
          .map((entry) => ({
            id: keyOf(entry),
            name: entry.name,
            description: entry.description,
            meta: <TransportMark transport={transportOf(entry)} />,
          })),
      };
    }
    if (domain === "model") {
      return {
        title: "添加 Model",
        mode: modelAgent?.supports_multiple ? "multiple" : "single",
        actionLabel: "添加 Model",
        busyLabel: "添加中…",
        emptyMessage: "没有可添加的兼容 Model",
        searchPlaceholder: "搜索 Model",
        options: modelProfiles
          .filter((profile) => !assigned.has(profile.id))
          .map((profile) => {
            const reason = modelCompatibilityReason(profile, modelAgent);
            return {
            id: profile.id,
            name: profile.name,
            description: profile.model,
            meta: <TransportMark transport={modelProtocolLabel(profile.protocol)} />,
            disabled: reason !== null,
            reason: reason ?? undefined,
          };}),
      };
    }
    return {
      title: "添加 Skill",
      mode: "multiple",
      actionLabel: "添加 Skill",
      busyLabel: "添加中…",
      emptyMessage: "没有可添加的 Skill",
      searchPlaceholder: "搜索 Skill",
      options: centralSkills.filter((item) => !assigned.has(item.name)).map((item) => ({
        id: item.name,
        name: item.name,
        description: item.description,
      })),
    };
  }

  const createSelection = (domain: AssetRef["domain"], ids: string[]): AgentConsumptionSelection => {
    if (domain === "mcp") return { domain, asset_keys: ids };
    if (domain === "model") return { domain, profile_ids: ids };
    return { domain, names: ids };
  };

  const planSelection = async (
    domain: AssetRef["domain"],
    ids: string[],
    commitWhenSafe: boolean,
  ) => {
    if (!consumptionState) return;
    setPreparingChange(true);
    try {
      const plan = await consumptionState.planForAgent(agentId, createSelection(domain, ids));
      setPickerDomain(null);
      if (commitWhenSafe && !requiresAgentReview(plan)) {
        await commitPlan(undefined, plan);
      }
    } catch (error) {
      showToast({ kind: "error", msg: "无法准备变更：" + formatError(error) });
    } finally {
      setPreparingChange(false);
    }
  };

  const planAdditions = (domain: PickerDomain, ids: string[]) => {
    const next = domain === "model" && modelAgent?.supports_multiple === false
      ? ids
      : [...new Set([...currentIds(domain), ...ids])].sort();
    return planSelection(domain, next, true);
  };

  const planRemoval = (asset: AssetRef) => {
    const id = asset.domain === "mcp" ? asset.key : asset.domain === "model" ? asset.profile_id : asset.name;
    return planSelection(
      asset.domain,
      currentIds(asset.domain).filter((candidate) => candidate !== id),
      false,
    );
  };

  const commitPlan = async (
    conflictConfirmation?: string,
    preparedPlan?: AssetOperationPlan,
    successMessage?: string,
  ) => {
    if (!consumptionState) return;
    const activePlan = preparedPlan ?? consumptionState.plan;
    try {
      await consumptionState.commit(conflictConfirmation);
      await Promise.all([
        rescan().catch(() => undefined),
        skillsState.refresh().catch(() => undefined),
        refreshModels().catch(() => undefined),
      ]);
      showToast({
        kind: "success",
        msg: successMessage
          ?? (activePlan ? completedMessage(activePlan, agent.name) : `${agent.name} 的配置已更新。`),
      });
    } catch (error) {
      showToast({ kind: "error", msg: "同步失败：" + formatError(error) });
    }
  };

  const toggleMcpEnabled = async (item: typeof mcpRows[number], enabled: boolean) => {
    if (!consumptionState || item.asset.domain !== "mcp") return;
    const key = item.asset.key;
    const name = entries.find((entry) => keyOf(entry) === key)?.name
      ?? key.replace(/::(?:stdio|http)$/, "");
    setTogglingMcp({ key, enabled });
    try {
      const plan = await consumptionState.planMcpEnabled(agentId, key, enabled);
      if (!requiresAgentReview(plan)) {
        await commitPlan(undefined, plan, `${name} 已${enabled ? "启用" : "停用"}。`);
      }
    } catch (error) {
      showToast({ kind: "error", msg: `${enabled ? "启用" : "停用"}失败：${formatError(error)}` });
    } finally {
      setTogglingMcp((current) => current?.key === key ? null : current);
    }
  };

  const toggleModelEnabled = async (item: typeof modelRows[number], enabled: boolean) => {
    if (!consumptionState || item.asset.domain !== "model") return;
    const profileId = item.asset.profile_id;
    const name = modelProfiles.find((profile) => profile.id === profileId)?.name ?? profileId;
    setChangingModel({ profileId, kind: "enabled", enabled });
    try {
      const plan = await consumptionState.planModelEnabled(agentId, profileId, enabled);
      if (!requiresAgentReview(plan)) {
        await commitPlan(undefined, plan, `${name} 已${enabled ? "启用" : "停用"}。`);
      }
    } catch (error) {
      showToast({ kind: "error", msg: `${enabled ? "启用" : "停用"}失败：${formatError(error)}` });
    } finally {
      setChangingModel((current) => current?.profileId === profileId ? null : current);
    }
  };

  const setActiveModel = async (item: typeof modelRows[number]) => {
    if (!consumptionState || item.asset.domain !== "model" || item.enabled === false || item.active) return;
    const profileId = item.asset.profile_id;
    const name = modelProfiles.find((profile) => profile.id === profileId)?.name ?? profileId;
    setChangingModel({ profileId, kind: "active" });
    try {
      const plan = await consumptionState.planActiveModel(agentId, profileId);
      if (!requiresAgentReview(plan)) {
        await commitPlan(undefined, plan, `${agent.name} 已切换到 ${name}。`);
      }
    } catch (error) {
      showToast({ kind: "error", msg: `切换失败：${formatError(error)}` });
    } finally {
      setChangingModel((current) => current?.profileId === profileId ? null : current);
    }
  };

  const openAsset = (asset: AssetRef) => {
    if (asset.domain === "mcp") {
      const split = asset.key.lastIndexOf("::");
      navigateResource({
        domain: "mcp",
        kind: "detail",
        name: split < 0 ? asset.key : asset.key.slice(0, split),
        transport: (split < 0 ? "stdio" : asset.key.slice(split + 2)) as "stdio" | "http",
      });
    } else if (asset.domain === "model") {
      navigateResource({ domain: "model", kind: "detail", profileId: asset.profile_id });
    } else {
      navigateResource({ domain: "skill", kind: "detail", skillName: asset.name });
    }
  };

  return (
    <div className="mux-agent-page">
      <div className="mux-agent-shell">
        <section className="mux-agent-context" aria-label={`${agent.name} 配置范围`}>
          <AgentHeader
            agent={agent}
            onEdit={agent.has_global ? () => setEditingAgent(true) : undefined}
          />
        </section>

        <AgentResourcePanel
          value={resourceTab}
          onChange={setResourceTab}
          counts={{
            mcps: mcpRows.length + mcpExternal.length,
            models: modelRows.length + modelExternalCards.length,
            skills: skillRows.length + skillExternal.length,
          }}
        >
          {resourceTab === "mcps" ? !agent.has_global ? (
            <div className="mux-agent-inline-state">此 Agent 未接入 MCP。</div>
          ) : (
            <AgentConsumptionPanel
              domain="mcp"
              title="MCP"
              description={`${mcpRows.length} 项`}
              manageLabel="添加 MCP"
              rows={displayedMcpRows}
              columns={3}
              external={mcpExternal}
              externalMode="cards"
              renderExternalAction={(item) => {
                if (item.asset.domain !== "mcp" || !onManageExternalMcp) return null;
                const assetKey = item.asset.key;
                return (
                  <button
                    type="button"
                    className="mux-consumption-adopt"
                    onClick={() => onManageExternalMcp(assetKey)}
                  >
                    让 MUX 管理
                  </button>
                );
              }}
              onManage={() => setPickerDomain("mcp")}
              manageDisabled={!agent.has_global || !consumptionState || preparingChange}
              onEnabledChange={(item, enabled) => void toggleMcpEnabled(item, enabled)}
              enabledChangeDisabled={(item) => !consumptionState
                || togglingMcp?.key === (item.asset.domain === "mcp" ? item.asset.key : "")
                || item.status !== "synced"}
              onRemove={(asset) => void planRemoval(asset)}
              removeLabel={(name) => `从 ${agent.name} 移除 ${name}`}
              removeDisabled={preparingChange}
              emptyTitle="暂无 MCP"
              present={(asset) => {
                const key = asset.domain === "mcp" ? asset.key : "";
                const entry = entries.find((candidate) => keyOf(candidate) === key);
                return {
                  name: entry?.name ?? key.replace(/::(?:stdio|http)$/, ""),
                  description: entry?.description || key,
                  icon: <Avatar seed={entry?.name ?? key} size={28} />,
                  meta: <TransportMark transport={entry ? transportOf(entry) : key.split("::").at(-1) ?? ""} />,
                };
              }}
            />
          ) : resourceTab === "models" ? (
            modelAgent?.mode === "guided" ? (
              <section className="mux-agent-section mux-agent-resource-content">
                <div className="mux-agent-guided-model">
                  <div><strong>由 Agent 管理</strong><span>请在 {agent.name} 内切换。</span></div>
                  <button type="button" className="btn-secondary" onClick={() => openUrl(modelAgent.docs)}>
                    <LinkIcon className="w-4 h-4" />打开设置文档
                  </button>
                </div>
              </section>
            ) : (
              modelsLoading ? (
                <div className="mux-agent-inline-state">正在读取 Model…</div>
              ) : modelsError ? (
                <div className="mux-agent-inline-state">Model 读取失败：{modelsError}</div>
              ) : !modelAgent ? (
                <div className="mux-agent-inline-state">此 Agent 尚未接入 Models。</div>
              ) : (
                <AgentConsumptionPanel
                  domain="model"
                  title="Models"
                  description={`${modelRows.length} 个已添加${modelAgent.supports_multiple ? " · 可保留多个并切换当前模型" : ""}`}
                  manageLabel="添加 Model"
                  rows={displayedModelRows}
                  columns={3}
                  external={modelExternalCards}
                  externalMode="cards"
                  renderExternalAction={(item) => {
                    if (item.asset.domain !== "model" || !onOpenMigration) return null;
                    const candidate = externalModelCandidateByProfileId.get(item.asset.profile_id);
                    if (!candidate) return null;
                    return (
                      <button
                        type="button"
                        className="mux-consumption-adopt"
                        onClick={() => onOpenMigration(modelMigrationCandidateId(candidate.fingerprint))}
                      >
                        让 MUX 管理
                      </button>
                    );
                  }}
                  onManage={() => setPickerDomain("model")}
                  manageDisabled={!consumptionState || preparingChange || compatibleProfiles.length === 0}
                  onOpenAsset={openAsset}
                  onEnabledChange={(item, enabled) => void toggleModelEnabled(item, enabled)}
                  enabledChangeDisabled={(item) => !consumptionState
                    || changingModel?.profileId === (item.asset.domain === "model" ? item.asset.profile_id : "")
                    || item.status === "conflicted"}
                  renderAction={(item) => item.desired_active ? (
                    <Badge tone={item.active === false ? "warning" : "success"}>
                      {item.active === false ? "期望当前" : "当前"}
                    </Badge>
                  ) : item.active ? (
                    <Badge tone="warning">Agent 实际当前</Badge>
                  ) : item.enabled === false ? null : (
                    <button
                      type="button"
                      className="mux-consumption-activate"
                      disabled={!consumptionState || changingModel !== null || item.status === "conflicted"}
                      onClick={() => void setActiveModel(item)}
                    >
                      设为当前
                    </button>
                  )}
                  onRemove={(asset) => void planRemoval(asset)}
                  removeLabel={(name) => `从 ${agent.name} 移除 ${name}`}
                  removeDisabled={preparingChange || changingModel !== null}
                  emptyTitle="暂无 Model"
                  emptyDescription={compatibleProfiles.length === 0
                    ? "模型库中没有兼容资产。"
                    : `从 Models 资产库添加到 ${agent.name}。`}
                  emptyAction={compatibleProfiles.length === 0 ? (
                    <button
                      type="button"
                      className="btn-secondary"
                      onClick={() => navigateResource({ domain: "model", kind: "create" })}
                    >
                      <PlusIcon className="w-4 h-4" />新建模型
                    </button>
                  ) : undefined}
                  present={(asset) => {
                    const profileId = asset.domain === "model" ? asset.profile_id : "";
                    const externalCandidate = externalModelCandidateByProfileId.get(profileId);
                    if (externalCandidate) {
                      return {
                        name: externalCandidate.name || externalCandidate.model,
                        description: `${externalCandidate.model} · ${modelProtocolLabel(externalCandidate.protocol)} · ${externalCandidate.provider}`,
                        icon: <Avatar seed={externalCandidate.name || externalCandidate.model} label="M" size={28} />,
                        meta: externalCandidate.active ? <Badge tone="warning">Agent 当前</Badge> : null,
                      };
                    }
                    const profile = modelProfiles.find((candidate) => candidate.id === profileId);
                    const credential = profile && modelAgent.credential_mode === "environment-reference"
                      ? profile.env_key
                        ? `ENV · ${profile.env_key}`
                        : profile.credential_saved ? "需要 ENV" : "无需凭据"
                      : profile?.credential_saved ? "Keychain" : "无需凭据";
                    return {
                      name: profile?.name ?? profileId,
                      description: profile
                        ? `${profile.model} · ${modelProtocolLabel(profile.protocol)} · ${credential}`
                        : "MUX 中央模型资产已缺失",
                      icon: <Avatar seed={profile?.name ?? profileId} label="M" size={28} />,
                    };
                  }}
                />
              )
            )
          ) : (
            <AgentConsumptionPanel
              domain="skill"
              title="Skills"
              description={`${skillRows.length} 项`}
              manageLabel="添加 Skill"
              rows={skillRows}
              columns={3}
              external={skillExternal}
              externalMode="cards"
              renderExternalAction={(item) => {
                if (item.asset.domain !== "skill" || !onOpenMigration) return null;
                const skillName = item.asset.name;
                return (
                  <button
                    type="button"
                    className="mux-consumption-adopt"
                    onClick={() => onOpenMigration(skillMigrationCandidateId(skillName))}
                  >
                    让 MUX 管理
                  </button>
                );
              }}
              onManage={() => setPickerDomain("skill")}
              onOpenAsset={openAsset}
              manageDisabled={!runtimeSkillAgent || !consumptionState || preparingChange}
              onRemove={(asset) => void planRemoval(asset)}
              removeLabel={(name) => `从 ${agent.name} 移除 ${name}`}
              removeDisabled={preparingChange}
              emptyTitle="暂无 Skill"
              present={(asset) => {
                const name = asset.domain === "skill" ? asset.name : "";
                const skill = centralSkills.find((candidate) => candidate.name === name);
                const externalSkill = (skillsState.inventory?.items ?? []).find(
                  (candidate) => candidate.name === name
                    && candidate.location.kind === "agent_target"
                    && candidate.states.includes("external")
                    && candidate.affected_agent_ids.includes(agentId),
                );
                const sharedCount = skillRows.find(
                  (row) => row.asset.domain === "skill" && row.asset.name === name,
                )?.affected_agent_ids.length ?? 0;
                return {
                  name,
                  description: skill?.description ?? externalSkill?.description ?? "Skill 资产已缺失",
                  icon: <SparklesIcon className="w-4 h-4" />,
                  meta: sharedCount > 1
                    ? <Badge tone="warning">共用 · {sharedCount}</Badge>
                    : null,
                };
              }}
            />
          )}
        </AgentResourcePanel>
      </div>

      {pickerDomain && picker && (
        <ConsumptionPickerDialog
          title={picker.title}
          mode={picker.mode}
          subtitle={agent.name}
          options={picker.options}
          actionLabel={picker.actionLabel}
          busyLabel={picker.busyLabel}
          emptyMessage={picker.emptyMessage}
          searchPlaceholder={picker.searchPlaceholder}
          onClose={() => setPickerDomain(null)}
          onSelect={(ids) => planAdditions(pickerDomain, ids)}
        />
      )}

      {consumptionState?.plan && !preparingChange && (
        <AssetOperationReviewDialog
          plan={consumptionState.plan}
          busy={consumptionState.committing}
          error={consumptionState.error?.message}
          agentId={agent.id}
          agentName={agent.name}
          onCommit={(conflictConfirmation) => commitPlan(conflictConfirmation)}
          onCancel={consumptionState.cancel}
        />
      )}

      {editingAgent && (
        <AgentConfigurationDialog
          agent={agent}
          modelAgent={modelAgent}
          onClose={() => setEditingAgent(false)}
          onSaved={async () => {
            await refreshAgents();
            await rescan();
            await refreshModels();
            await skillsState.refresh();
            await consumptionState?.refresh();
          }}
        />
      )}
    </div>
  );
}

function TransportMark({ transport }: { transport: string }) {
  return <span className="mux-transport-mark">{transport}</span>;
}

function AgentHeader({
  agent,
  tone,
  onEdit,
}: {
  agent: InstallState["agents"][number];
  tone?: "reference";
  onEdit?: () => void;
}) {
  return (
    <header
      className="mux-agent-header"
      data-tone={tone}
      aria-label={`Agent ${agent.name} (${agent.id})`}
    >
      <div className="mux-agent-header-identity">
        <AgentGlyph id={agent.id} name={agent.name} size={44} />
        <div className="mux-agent-header-copy">
          <div>
            <h2>{agent.name}</h2>
            {tone === "reference" ? <Badge>仅供参考</Badge> : agent.evidence === "community-extension" ? (
              <Badge tone="warning">社区扩展</Badge>
            ) : agent.builtin ? <Badge tone="success">已核验</Badge> : <Badge>自定义</Badge>}
          </div>
        </div>
      </div>
      {(onEdit || agent.docs) && (
        <div className="mux-agent-header-actions">
          {onEdit && (
            <IconButton title="编辑 Agent 设置" onClick={onEdit}>
              <EditIcon className="w-4 h-4" />
            </IconButton>
          )}
          {agent.docs && (
            <IconButton title="打开官方文档" onClick={() => openUrl(agent.docs!)}>
              <LinkIcon className="w-4 h-4" />
            </IconButton>
          )}
        </div>
      )}
    </header>
  );
}
