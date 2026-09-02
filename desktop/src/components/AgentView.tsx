import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { homeDir } from "@tauri-apps/api/path";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import type { InstallState } from "../hooks/useInstallState";
import type { SkillsState } from "../hooks/useSkillsState";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import { useMcpIconPreferences } from "../hooks/useMcpIconPreferences";
import type {
  AgentConsumptionSelection,
  ApiKeyDelivery,
  AssetOperationPlan,
  AssetRef,
  ConsumptionView,
  ConvergenceAction,
  ModelAdoptionCandidate,
  ModelAgentView,
  ModelProfileView,
  OperationPlan,
  ResourceNavigationRequest,
} from "../lib/types";
import { formatError } from "../lib/format";
import { keyOf, transportOf } from "../lib/mcp";
import { consumptionsForAgent, externalForAgent } from "../lib/consumption";
import { requiresAgentReview } from "../lib/agentOperation";
import { listModelAgents, listModelProfiles, setModelCredentialDelivery } from "../lib/api";
import {
  EditIcon,
  ExternalLinkIcon,
  LayersIcon,
  LinkIcon,
  PackageIcon,
  PlusIcon,
  RefreshIcon,
  SparklesIcon,
} from "./icons";
import { Avatar, Badge } from "./ui";
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
import { ReviewDialog } from "./ReviewDialog";
import { mergeAgentInfos } from "../lib/agentCapabilities";
import { SkillReviewDialog } from "./SkillReviewDialog";
import { useTranslation } from "react-i18next";
import { McpAvatar } from "./McpIcon";
import { FormSelect } from "./FormSelect";
import { DialogShell } from "./DialogShell";

type PickerDomain = "mcp" | "model" | "skill";
type ConfigLocationKind = "file" | "folder";

function absoluteConfigLocation(path: string, home: string) {
  const value = path.trim();
  const normalizedHome = home.replace(/\/$/, "");
  if (value === "~" && normalizedHome) return normalizedHome;
  if (value.startsWith("~/") && normalizedHome) return `${normalizedHome}/${value.slice(2)}`;
  return value;
}

function configLocations(paths: string[] | undefined, fallback?: string | null) {
  const values = (paths ?? []).map((path) => path.trim()).filter(Boolean);
  if (values.length > 0) return [...new Set(values)];
  return [...new Set((fallback ?? "").split(/\s+(?:\+|·)\s+/).map((path) => path.trim()).filter(Boolean))];
}

function modelProtocolLabel(protocol: ModelProfileView["protocol"]) {
  if (protocol === "anthropic-messages") return "Anthropic Messages";
  if (protocol === "openai-responses") return "OpenAI Responses";
  if (protocol === "gemini-generate-content") return "Gemini GenerateContent";
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
  consumptionState: ConsumptionState;
  agentId: string;
  externalModelCandidates?: ModelAdoptionCandidate[];
  onOpenResource?(request: ResourceNavigationRequest): void;
}

function completedMessage(plan: AssetOperationPlan, agentName: string) {
  if (plan.kind === "clear-mcp") return `${agentName} 的全部 MCP 已移除。`;
  if (plan.kind === "clear-models") return `${agentName} 的全部 Model 已从权威配置中移除。`;
  const domain = plan.domain_plan.domain;
  const asset = domain === "mcp" ? "MCP" : domain === "model" ? "Model" : "Skill";
  const hasAdd = plan.relationship_changes.some((change) => change.action === "add");
  const hasRemove = plan.relationship_changes.some((change) => change.action === "remove");
  const removesEveryModel = plan.domain_plan.domain === "model"
    && Object.values(plan.domain_plan.after).every(
      (selection) => Object.keys(selection.profiles).length === 0,
    );
  if (domain === "model" && hasAdd) return `Model 已添加到 ${agentName}。`;
  if (hasAdd && !hasRemove) return `${asset} 已添加到 ${agentName}。`;
  if (hasRemove && !hasAdd && removesEveryModel) return `${agentName} 的全部 Model 已移除。`;
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

export function AgentView({
  state,
  skillsState,
  consumptionState,
  agentId,
  externalModelCandidates = [],
  onOpenResource,
}: AgentViewProps) {
  const { t } = useTranslation();
  const { entries, refreshAgents } = state;
  const { show: showToast } = useToast();
  const mcpIcons = useMcpIconPreferences();
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
  const [togglingAllMcp, setTogglingAllMcp] = useState<{ enabled: boolean } | null>(null);
  const [togglingSkill, setTogglingSkill] = useState<{
    name: string;
    enabled: boolean;
  } | null>(null);
  const [changingModel, setChangingModel] = useState<{ profileId: string } | null>(null);
  const [changingCredential, setChangingCredential] = useState<string | null>(null);
  const [plaintextConfirmation, setPlaintextConfirmation] = useState<{
    profileId: string;
    profileName: string;
    target: string;
  } | null>(null);
  const [skillConvergencePlan, setSkillConvergencePlan] = useState<OperationPlan | null>(null);
  const [userHome, setUserHome] = useState("");
  const visibleIncidents = useMemo(() => {
    const capability = resourceTab === "mcps" ? "mcp" : resourceTab === "models" ? "model" : "skill";
    return (consumptionState.inventory?.target_incidents ?? []).filter(
      (incident) => incident.capability === capability
        && incident.affected_agent_ids.includes(agentId),
    );
  }, [agentId, consumptionState.inventory?.target_incidents, resourceTab]);

  const navigateResource = useCallback((request: ResourceNavigationRequest) => {
    onOpenResource?.(request);
  }, [onOpenResource]);

  useEffect(() => {
    let active = true;
    homeDir().then((path) => {
      if (active) setUserHome(path);
    }).catch(() => undefined);
    return () => {
      active = false;
    };
  }, []);

  const agents = useMemo(
    () => mergeAgentInfos(state.agents, consumptionState.agents),
    [consumptionState.agents, state.agents],
  );
  const agent = useMemo(
    () => agents.find((item) => item.id === agentId) ?? null,
    [agentId, agents],
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
  const canEditConfiguration = Boolean(
    agent?.has_global || modelAgent !== null || agent?.skills_global_dir,
  );
  const compatibleProfiles = useMemo(
    () => modelAgent
      ? modelProfiles.filter((profile) => modelCompatibilityReason(profile, modelAgent) === null)
      : [],
    [modelAgent, modelProfiles],
  );
  const applyCredentialDelivery = useCallback(async (
    profileId: string,
    delivery: ApiKeyDelivery,
    confirmPlaintext = false,
  ) => {
    setChangingCredential(profileId);
    try {
      await setModelCredentialDelivery(agentId, profileId, delivery, confirmPlaintext);
      await Promise.all([refreshModels(), consumptionState.refresh()]);
      showToast({ kind: "success", msg: "API Key 写入方式已更新。" });
    } catch (error) {
      showToast({ kind: "error", msg: `更新 API Key 写入方式失败：${formatError(error)}` });
    } finally {
      setChangingCredential(null);
    }
  }, [agentId, consumptionState, refreshModels, showToast]);
  const inventory = consumptionState.inventory;
  const mcpRows = consumptionsForAgent(inventory, agentId, "mcp");
  const modelRows = consumptionsForAgent(inventory, agentId, "model");
  const skillRows = consumptionsForAgent(inventory, agentId, "skill");
  const displayedMcpRows = useMemo(
    () => mcpRows.map((item) => (
      togglingAllMcp
        ? { ...item, enabled: togglingAllMcp.enabled }
        : togglingMcp && item.asset.domain === "mcp" && item.asset.key === togglingMcp.key
          ? { ...item, enabled: togglingMcp.enabled }
          : item
    )),
    [mcpRows, togglingAllMcp, togglingMcp],
  );
  const displayedSkillRows = useMemo(
    () => skillRows.map((item) => (
      togglingSkill && item.asset.domain === "skill" && item.asset.name === togglingSkill.name
        ? { ...item, enabled: togglingSkill.enabled }
        : item
    )),
    [skillRows, togglingSkill],
  );
  const mcpExternal = externalForAgent(inventory, agentId, "mcp");
  const skillExternal = externalForAgent(inventory, agentId, "skill");
  const modelExternal = externalForAgent(inventory, agentId, "model");
  const authorityModelRows = modelAgent?.storage_authority === "native-registry"
    ? modelRows.filter((item) => item.observed)
    : modelRows;
  const modelConfiguredCount = authorityModelRows.length + modelExternal.length;
  const modelVisibleCount = modelAgent?.storage_authority === "native-registry"
    ? modelConfiguredCount
    : modelRows.length;
  const agentModelMigrationCandidates = useMemo(
    () => externalModelCandidates
      .filter((candidate) => candidate.agent_id === agentId)
      .sort((left, right) => Number(right.active) - Number(left.active)
        || (left.name || left.model).localeCompare(right.name || right.model)),
    [agentId, externalModelCandidates],
  );
  const displayedModelRows = useMemo(
    () => authorityModelRows.map((item) => {
      const profileId = item.asset.domain === "model" ? item.asset.profile_id : "";
      const current = changingModel
        ? profileId === changingModel.profileId
        : item.desired_active ?? item.active ?? false;
      return { ...item, enabled: current };
    }),
    [authorityModelRows, changingModel],
  );

  if (!agent) return <div className="mux-agent-state">未找到该 Agent</div>;

  if (!agent.has_global && !agent.skills_global_dir && !modelsLoading && !modelAgent) {
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

  const mcpConfigPaths = agent.has_global && agent.global ? [agent.global] : [];
  const mcpDescription = agent.has_global
    ? `${agent.format.toUpperCase()} · ${agent.key}`
    : "此 Agent 未接入 MCP";
  const skillsConfigPaths = configLocations(agent.skills_global_dirs, agent.skills_global_dir);
  const modelConfigPaths = configLocations(modelAgent?.config_paths, modelAgent?.config_path);
  const runtimeSkillAgent = skillsState.inventory?.agents.find((item) => item.id === agentId) ?? null;
  const modelDescription = modelsLoading
    ? "读取中…"
    : modelsError
      ? "读取失败"
      : modelAgent?.mode === "guided"
        ? "Agent 内管理"
        : modelAgent?.storage_authority === "native-registry"
          ? `真实配置${modelAgent.supports_multiple ? " · 多模型" : ""}`
          : modelAgent ? "MUX 映射" : "未接入";
  const skillsDescription = skillsConfigPaths.length === 0
    ? "未接入"
    : skillsState.loading
      ? "读取中…"
      : skillsState.error
        ? "读取失败"
        : runtimeSkillAgent && runtimeSkillAgent.affected_agent_ids.length > 1
          ? `用户目录 · 共用 ${runtimeSkillAgent.affected_agent_ids.length}`
          : "用户目录";

  const centralSkills = (skillsState.inventory?.items ?? []).filter(
    (item) => item.location.kind === "central" && item.states.includes("managed"),
  );
  const agentDisplayNames = Object.fromEntries(
    agents.map((item) => [item.id, item.name]),
  );
  const assetDisplayNames = Object.fromEntries([
    ...entries.map((entry) => [`mcp:${keyOf(entry)}`, entry.name] as const),
    ...modelProfiles.map((profile) => [`model:${profile.id}`, profile.name] as const),
    ...centralSkills.map((skill) => [`skill:${skill.name}`, skill.name] as const),
  ]);

  const openConfigLocation = async (path: string, kind: ConfigLocationKind) => {
    try {
      const home = userHome || await homeDir();
      await openPath(absoluteConfigLocation(path, home));
    } catch (error) {
      showToast({
        kind: "error",
        msg: `无法打开${kind === "folder" ? "文件夹" : "文件"}：${formatError(error)}`,
      });
    }
  };

  const currentIds = (domain: PickerDomain): string[] => {
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
            icon: <McpAvatar
              assetKey={keyOf(entry)}
              entry={entry}
              preference={mcpIcons.preferences[keyOf(entry)]}
              size={28}
            />,
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
            };
          }),
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

  const createSelection = (domain: PickerDomain, ids: string[]): AgentConsumptionSelection => {
    if (domain === "mcp") return { domain, asset_keys: ids };
    if (domain === "model") return { domain, profile_ids: ids };
    return { domain, names: ids };
  };

  const planSelection = async (
    domain: PickerDomain,
    ids: string[],
    mode: "add" | "replace" | "remove",
  ) => {
    setPreparingChange(true);
    try {
      const plan = mode === "add"
        ? await consumptionState.planAdditionsForAgent(agentId, createSelection(domain, ids))
        : await consumptionState.planForAgent(agentId, createSelection(domain, ids));
      setPickerDomain(null);
      if (!requiresAgentReview(plan)) {
        await commitPlan(plan);
      }
    } catch (error) {
      showToast({ kind: "error", msg: "无法准备变更：" + formatError(error) });
    } finally {
      setPreparingChange(false);
    }
  };

  const planAdditions = (domain: PickerDomain, ids: string[]) => {
    const replacesSingleModel = domain === "model" && modelAgent?.supports_multiple === false;
    return planSelection(domain, ids, replacesSingleModel ? "replace" : "add");
  };

  const planRemoval = (asset: AssetRef) => {
    if (asset.domain === "model-provider") return;
    const id = asset.domain === "mcp" ? asset.key : asset.domain === "model" ? asset.profile_id : asset.name;
    return planSelection(
      asset.domain,
      currentIds(asset.domain).filter((candidate) => candidate !== id),
      "remove",
    );
  };

  const clearModels = async () => {
    setPreparingChange(true);
    try {
      await consumptionState.planClearAgentModels(agentId);
    } catch (error) {
      showToast({ kind: "error", msg: "无法准备清空 Models：" + formatError(error) });
    } finally {
      setPreparingChange(false);
    }
  };

  const commitClearModels = async () => {
    await consumptionState.commit();
    showToast({
      kind: "success",
      msg: `${agent.name} 的全部 Model 已从权威配置中移除。`,
    });
  };

  const clearMcp = async () => {
    setPreparingChange(true);
    try {
      await consumptionState.clearAgentMcp(agentId);
      showToast({ kind: "success", msg: `${agent.name} 的全部 MCP 已移除。` });
    } catch (error) {
      showToast({ kind: "error", msg: "移除全部 MCP 失败：" + formatError(error) });
    } finally {
      setPreparingChange(false);
    }
  };

  const commitPlan = async (
    preparedPlan?: AssetOperationPlan,
    successMessage?: string,
  ) => {
    const activePlan = preparedPlan ?? consumptionState.plan;
    try {
      await consumptionState.commit();
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
    if (item.asset.domain !== "mcp") return;
    const key = item.asset.key;
    const name = entries.find((entry) => keyOf(entry) === key)?.name
      ?? key.replace(/::(?:stdio|http)$/, "");
    setTogglingMcp({ key, enabled });
    try {
      await consumptionState.setMcpEnabled(agentId, key, enabled);
      showToast({ kind: "success", msg: `${name} 已${enabled ? "启用" : "停用"}。` });
    } catch (error) {
      showToast({ kind: "error", msg: `${enabled ? "启用" : "停用"}失败：${formatError(error)}` });
    } finally {
      setTogglingMcp((current) => current?.key === key ? null : current);
    }
  };

  const toggleAllMcpEnabled = async (enabled: boolean) => {
    setTogglingAllMcp({ enabled });
    try {
      await consumptionState.setAllMcpEnabled(agentId, enabled);
      showToast({
        kind: "success",
        msg: `${agent.name} 的全部 MCP 已${enabled ? "启用" : "停用"}。`,
      });
    } catch (error) {
      showToast({
        kind: "error",
        msg: `${enabled ? "启用" : "停用"}全部 MCP 失败：${formatError(error)}`,
      });
    } finally {
      setTogglingAllMcp(null);
    }
  };

  const toggleSkillEnabled = async (item: typeof skillRows[number], enabled: boolean) => {
    if (item.asset.domain !== "skill") return;
    const name = item.asset.name;
    setTogglingSkill({ name, enabled });
    try {
      await consumptionState.setSkillEnabled(agentId, name, enabled);
      showToast({ kind: "success", msg: `${name} 已${enabled ? "启用" : "停用"}。` });
    } catch (error) {
      showToast({ kind: "error", msg: `${enabled ? "启用" : "停用"}失败：${formatError(error)}` });
    } finally {
      setTogglingSkill((current) => current?.name === name ? null : current);
    }
  };

  const setActiveModel = async (item: typeof modelRows[number]) => {
    if (item.asset.domain !== "model" || item.desired_active) return;
    const profileId = item.asset.profile_id;
    const name = modelProfiles.find((profile) => profile.id === profileId)?.name ?? profileId;
    setChangingModel({ profileId });
    try {
      await consumptionState.setActiveModel(agentId, profileId);
      showToast({ kind: "success", msg: `${agent.name} 已切换到 ${name}。` });
    } catch (error) {
      showToast({ kind: "error", msg: `切换失败：${formatError(error)}` });
    } finally {
      setChangingModel((current) => current?.profileId === profileId ? null : current);
    }
  };

  const switchActiveModel = (item: typeof modelRows[number], current: boolean) => {
    if (current) return void setActiveModel(item);
    showToast({ kind: "error", msg: "请先选择其他当前 Model。" });
  };

  const converge = async (item: ConsumptionView, action: ConvergenceAction) => {
    if (preparingChange) return;
    setPreparingChange(true);
    try {
      const result = await consumptionState.planConvergence(item, action);
      if (result.domain === "skill") setSkillConvergencePlan(result.plan);
    } catch (error) {
      showToast({
        kind: "error",
        msg: t("observations.convergencePrepareFailed", { error: formatError(error) }),
      });
    } finally {
      setPreparingChange(false);
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
    } else if (asset.domain === "skill") {
      navigateResource({ domain: "skill", kind: "detail", skillName: asset.name });
    }
  };

  return (
    <div className="mux-agent-page">
      <div className="mux-agent-shell">
        <section className="mux-agent-context" aria-label={`${agent.name} 配置范围`}>
          <AgentHeader agent={agent} />

          <section
            className="mux-agent-section mux-agent-config-locations"
            aria-labelledby="agent-files-title"
            aria-label="配置位置"
          >
            <div className="mux-agent-section-head">
              <div>
                <h3 id="agent-files-title">配置位置</h3>
                <p>这些是 MUX 为当前 Agent 读取或写入的实际位置</p>
              </div>
              {(agent.docs || canEditConfiguration) && (
                <div className="mux-agent-section-actions">
                  {agent.docs && (
                    <button type="button" className="btn-secondary" onClick={() => openUrl(agent.docs!)}>
                      <LinkIcon className="w-3.5 h-3.5" />官方文档
                    </button>
                  )}
                  {canEditConfiguration && (
                    <button type="button" className="btn-secondary" onClick={() => setEditingAgent(true)}>
                      <EditIcon className="w-3.5 h-3.5" />编辑配置
                    </button>
                  )}
                </div>
              )}
            </div>
            <div className="mux-agent-file-map">
              <ConfigPath
                icon={<PackageIcon className="w-4 h-4" />}
                label="MCPs"
                description={mcpDescription}
                paths={mcpConfigPaths}
                kind="file"
                home={userHome}
                onOpen={openConfigLocation}
                unavailableLabel={agent.has_global ? undefined : "未接入"}
              />
              <ConfigPath
                icon={<LayersIcon className="w-4 h-4" />}
                label="Models"
                description={modelDescription}
                paths={modelConfigPaths}
                kind="file"
                home={userHome}
                onOpen={openConfigLocation}
              />
              <ConfigPath
                icon={<SparklesIcon className="w-4 h-4" />}
                label="Skills"
                description={skillsDescription}
                paths={skillsConfigPaths}
                kind="folder"
                home={userHome}
                onOpen={openConfigLocation}
              />
            </div>
          </section>
        </section>

        <AgentResourcePanel
          value={resourceTab}
          onChange={setResourceTab}
          counts={{
            mcps: mcpRows.length + mcpExternal.length,
            models: modelVisibleCount,
            skills: skillRows.length + skillExternal.length,
          }}
        >
          {consumptionState.plan?.kind === "clear-models" && !preparingChange && (
            <ReviewDialog
              title="清空全部 Models"
              subtitle={`${agent.name} · ${modelVisibleCount} 个 Models`}
              confirmLabel={`清空 ${modelVisibleCount} 个 Models`}
              onConfirm={commitClearModels}
              onClose={() => void consumptionState.cancel()}
            >
              <div className="mux-clear-models-impact">
                <strong>将移除此 Agent 配置中的全部 Model</strong>
                <span>包括外部和手工配置，操作无法撤销</span>
              </div>
              <p className="mux-clear-models-preserved">
                中央 Models、Providers 与凭据保持不变
              </p>
            </ReviewDialog>
          )}
          {consumptionState.plan
            && consumptionState.plan.kind !== "clear-models"
            && !preparingChange ? (
            <AssetOperationReviewDialog
              plan={consumptionState.plan}
              busy={consumptionState.committing}
              error={consumptionState.error}
              agentId={agent.id}
              agentName={agent.name}
              agentDisplayNames={agentDisplayNames}
              assetDisplayNames={assetDisplayNames}
              onCommit={() => commitPlan()}
              onCancel={consumptionState.cancel}
            />
          ) : <>
          {visibleIncidents.map((incident) => (
            <div className="mux-target-incident" role="status" key={incident.id}>
              <div>
                <strong>{t("observations.targetIncidentTitle")}</strong>
                <span>{t("observations.targetIncidentMessage")}</span>
              </div>
              <code title={incident.target_path}>{incident.target_path}</code>
            </div>
          ))}
          {preparingChange && (
            <div className="mux-agent-operation-progress" role="status" aria-live="polite">
              <RefreshIcon data-spinning="true" />
              <span>正在检查并同步 {agent.name} 的资产…</span>
            </div>
          )}
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
              onManage={() => setPickerDomain("mcp")}
              manageDisabled={!agent.has_global || preparingChange}
              bulkRemoveLabel="移除全部 MCP"
              bulkRemoveTitle={`清空 ${agent.name} 的全部 MCP，包括外部配置；不会删除中央 MCP 资产`}
              bulkRemoveDisabled={preparingChange || togglingAllMcp !== null || mcpRows.length + mcpExternal.length === 0}
              onBulkRemove={() => void clearMcp()}
              bulkToggleLabel="全部"
              bulkEnabled={mcpRows.length > 0 && displayedMcpRows.every((item) => item.enabled === true)}
              bulkToggleDisabled={preparingChange || togglingMcp !== null || togglingAllMcp !== null || mcpRows.length === 0}
              onBulkEnabledChange={(enabled) => void toggleAllMcpEnabled(enabled)}
              onEnabledChange={(item, enabled) => void toggleMcpEnabled(item, enabled)}
              enabledChangeDisabled={(item) => togglingAllMcp !== null
                || togglingMcp?.key === (item.asset.domain === "mcp" ? item.asset.key : "")
                || item.status !== "synced"}
              onRemove={(asset) => void planRemoval(asset)}
              onConverge={(item, action) => void converge(item, action)}
              convergenceDisabled={preparingChange}
              removeLabel={(name) => `从 ${agent.name} 移除 ${name}`}
              removeDisabled={preparingChange}
              emptyTitle="暂无 MCP"
              present={(asset) => {
                const key = asset.domain === "mcp" ? asset.key : "";
                const entry = entries.find((candidate) => keyOf(candidate) === key);
                const iconEntry = entry ?? {
                  name: key.replace(/::(?:stdio|http)$/, ""),
                  description: "",
                  tags: [],
                  config: {},
                };
                return {
                  name: iconEntry.name,
                  description: entry?.description?.trim() || undefined,
                  icon: <McpAvatar assetKey={key} entry={iconEntry} preference={mcpIcons.preferences[key]} size={28} />,
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
                  description={modelAgent.storage_authority === "native-registry"
                    ? `配置中 ${modelVisibleCount} 个${modelAgent.supports_multiple ? " · 同一时间使用其中一个" : ""}`
                    : `MUX 管理 ${modelVisibleCount} 个`}
                  manageLabel="添加 Model"
                  rows={displayedModelRows}
                  columns={3}
                  external={modelAgent.storage_authority === "native-registry" ? modelExternal : []}
                  externalMode="cards"
                  bulkRemoveLabel="清空全部 Models"
                  bulkRemoveTitle={modelAgent.storage_authority === "native-registry"
                    ? `清空 ${agent.name} 真实配置中的全部 Model，包括外部和手工配置；中央资产与凭据保留`
                    : `清空 ${agent.name} 的全部 MUX Model 映射；中央资产与凭据保留`}
                  bulkRemoveDisabled={modelVisibleCount === 0
                    || preparingChange
                    || consumptionState.committing
                    || changingModel !== null}
                  onBulkRemove={() => void clearModels()}
                  onManage={() => setPickerDomain("model")}
                  manageDisabled={preparingChange || compatibleProfiles.length === 0}
                  onOpenAsset={openAsset}
                  onEnabledChange={switchActiveModel}
                  toggleKind="current"
                  enabledChangeDisabled={(item) => changingModel !== null
                    || item.status === "ambiguous"}
                  renderAction={(item) => {
                    const profileId = item.asset.domain === "model" ? item.asset.profile_id : "";
                    const profile = modelProfiles.find((candidate) => candidate.id === profileId);
                    const options = [
                      { value: "auto", label: "自动" },
                      ...(modelAgent.credential_capabilities?.agent_store
                        ? [{ value: "agent-store", label: "Agent 凭据存储" }]
                        : []),
                      ...(modelAgent.credential_capabilities?.plaintext && agentId === "opencode"
                        ? [{ value: "plaintext", label: "明文配置" }]
                        : []),
                    ];
                    const delivery = modelAgent.credential_policies?.[profileId]?.delivery ?? "auto";
                    return (
                      <div className="mux-model-row-actions" data-busy={changingCredential === profileId ? "true" : undefined}>
                        {item.desired_active ? (
                          <Badge tone={item.active === false ? "warning" : "success"}>
                            {item.active === false ? "期望当前" : "当前"}
                          </Badge>
                        ) : item.active ? (
                          <Badge tone="warning">Agent 实际当前</Badge>
                        ) : null}
                        <FormSelect
                          ariaLabel={`${profile?.name ?? profileId} API Key 写入方式`}
                          value={delivery}
                          options={options}
                          onChange={(value) => {
                            const next = value as ApiKeyDelivery;
                            if (next === "plaintext") {
                              setPlaintextConfirmation({
                                profileId,
                                profileName: profile?.name ?? profileId,
                                target: modelAgent.config_paths[0] ?? modelAgent.config_path,
                              });
                            } else {
                              void applyCredentialDelivery(profileId, next);
                            }
                          }}
                        />
                      </div>
                    );
                  }}
                  onRemove={(asset) => void planRemoval(asset)}
                  onConverge={(item, action) => void converge(item, action)}
                  convergenceDisabled={preparingChange}
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
                      <PlusIcon className="w-4 h-4" />添加模型
                    </button>
                  ) : undefined}
                  present={(asset) => {
                    const profileId = asset.domain === "model" ? asset.profile_id : "";
                    const externalCandidate = profileId.startsWith("external-")
                      ? agentModelMigrationCandidates.find(
                        (candidate) => profileId === `external-${candidate.candidate_id}`,
                      )
                      : undefined;
                    if (externalCandidate) {
                      return {
                        name: externalCandidate.name || externalCandidate.model,
                        description: `${externalCandidate.model} · ${modelProtocolLabel(externalCandidate.protocol)} · ${externalCandidate.provider}`,
                        icon: <Avatar seed={externalCandidate.name || externalCandidate.model} kind="model" size={28} />,
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
                      icon: <Avatar seed={profile?.name ?? profileId} kind="model" size={28} />,
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
              rows={displayedSkillRows}
              columns={3}
              external={skillExternal}
              externalMode="cards"
              onManage={() => setPickerDomain("skill")}
              onOpenAsset={openAsset}
              manageDisabled={!runtimeSkillAgent || preparingChange}
              onEnabledChange={(item, enabled) => void toggleSkillEnabled(item, enabled)}
              enabledChangeDisabled={(item) => togglingSkill?.name === (item.asset.domain === "skill" ? item.asset.name : "")
                || item.status !== "synced"}
              onRemove={(asset) => void planRemoval(asset)}
              onConverge={(item, action) => void converge(item, action)}
              convergenceDisabled={preparingChange}
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
                  icon: <Avatar seed={name} kind="skill" size={28} />,
                  meta: sharedCount > 1
                    ? <Badge tone="warning">共用 · {sharedCount}</Badge>
                    : null,
                };
              }}
            />
          )}
          </>}
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

      {skillConvergencePlan && (
        <SkillReviewDialog
          plan={skillConvergencePlan}
          onCommit={skillsState.commit}
          onClose={async () => {
            await skillsState.cancel(skillConvergencePlan.operation_id);
            setSkillConvergencePlan(null);
          }}
          onCommitted={async (next) => {
            skillsState.hydrate(next);
            setSkillConvergencePlan(null);
            await consumptionState.refresh();
          }}
          onRecoveryRequired={(message) => {
            showToast({ kind: "error", msg: message });
          }}
        />
      )}

      {editingAgent && (
        <AgentConfigurationDialog
          agent={agent}
          modelAgent={modelAgent}
          onClose={() => setEditingAgent(false)}
          onSaved={async () => {
            await Promise.allSettled([
              refreshAgents(),
              refreshModels(),
              consumptionState.refreshAgents(),
              consumptionState.refresh(),
              skillsState.refresh(),
            ]);
          }}
        />
      )}

      {plaintextConfirmation && (
        <DialogShell
          kind="review"
          size="sm"
          className="mux-plaintext-confirmation"
          title="明文写入 API Key"
          subtitle={agent.name}
          busy={changingCredential === plaintextConfirmation.profileId}
          onClose={() => setPlaintextConfirmation(null)}
          footerEnd={(
            <>
              <button type="button" className="btn-secondary" onClick={() => setPlaintextConfirmation(null)}>
                取消
              </button>
              <button
                type="button"
                className="btn-danger"
                onClick={async () => {
                  const pending = plaintextConfirmation;
                  await applyCredentialDelivery(pending.profileId, "plaintext", true);
                  setPlaintextConfirmation(null);
                }}
              >
                明文写入
              </button>
            </>
          )}
        >
          <div className="mux-plaintext-confirmation-body">
            <strong>将把 {plaintextConfirmation.profileName} API Key 明文写入 {agent.name} 私有配置</strong>
            <code>{plaintextConfirmation.target}</code>
            <span>文件权限将设为 0600</span>
          </div>
        </DialogShell>
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
}: {
  agent: InstallState["agents"][number];
  tone?: "reference";
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
            ) : !agent.builtin ? <Badge>自定义</Badge> : null}
          </div>
        </div>
      </div>
    </header>
  );
}

function ConfigPath({
  icon,
  label,
  description,
  paths,
  kind,
  home,
  onOpen,
  unavailableLabel = "不可用",
}: {
  icon: ReactNode;
  label: string;
  description: string;
  paths: string[];
  kind: ConfigLocationKind;
  home: string;
  onOpen(path: string, kind: ConfigLocationKind): Promise<unknown> | unknown;
  unavailableLabel?: string;
}) {
  return (
    <div className="mux-agent-file-row">
      <span className="mux-agent-file-icon">{icon}</span>
      <div className="mux-agent-file-copy">
        <div><strong>{label}</strong><span>{description}</span></div>
        {paths.length > 0 ? (
          <div className="mux-agent-file-paths">
            {paths.map((path) => {
              const absolutePath = absoluteConfigLocation(path, home);
              return (
                <button
                  type="button"
                  className="mux-agent-file-path"
                  key={path}
                  title={kind === "folder" ? `在 Finder 中打开 ${absolutePath}` : `使用默认应用打开 ${absolutePath}`}
                  aria-label={`${kind === "folder" ? "打开文件夹" : "使用默认应用打开文件"}：${absolutePath}`}
                  onClick={() => void onOpen(path, kind)}
                >
                  <code>{absolutePath}</code>
                  <ExternalLinkIcon className="w-3 h-3" />
                </button>
              );
            })}
          </div>
        ) : (
          <span className="mux-agent-file-unavailable">{unavailableLabel}</span>
        )}
      </div>
    </div>
  );
}
