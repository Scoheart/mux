import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { InstallState } from "../hooks/useInstallState";
import type { SkillsState } from "../hooks/useSkillsState";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import type {
  AgentConsumptionSelection,
  AssetRef,
  ConsumptionInventory,
  ModelAgentView,
  ModelProfileView,
  ResourceNavigationRequest,
} from "../lib/types";
import { formatError } from "../lib/format";
import { keyOf, transportOf } from "../lib/mcp";
import { consumptionsForAgent, externalForAgent } from "../lib/consumption";
import { listModelAgents, listModelProfiles } from "../lib/api";
import { EditIcon, LayersIcon, LinkIcon, PackageIcon, SparklesIcon } from "./icons";
import { Avatar, Badge, IconButton, TransportPill } from "./ui";
import { AgentGlyph } from "./brandIcons";
import { AddAgentDialog } from "./AddAgentDialog";
import { useToast } from "./Toast";
import { AgentResourcePanel, type AgentResourceTab } from "./AgentResourcePanel";
import { AgentConsumptionPanel } from "./AgentConsumptionPanel";
import {
  ConsumptionPickerDialog,
  type ConsumptionPickerOption,
} from "./ConsumptionPickerDialog";
import { AssetOperationReviewDialog } from "./AssetOperationReviewDialog";

interface AgentViewProps {
  state: InstallState;
  skillsState: SkillsState;
  consumptionState?: ConsumptionState;
  agentId: string;
  onOpenResource?(request: ResourceNavigationRequest): void;
  /** Transitional test adapters. */
  onOpenModels?: () => void;
  onOpenSkills?: (request: Extract<ResourceNavigationRequest, { domain: "skill" }>) => void;
}

function samePath(left: string, right: string) {
  return left.trim().replace(/\/+$/, "") === right.trim().replace(/\/+$/, "");
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
          status: item.customized ? "drifted" as const : "synced" as const,
          reason: item.customized ? "mcp_config_drift" : null,
          affected_agent_ids: [item.agent],
        })),
      ...modelAgents.flatMap((item) =>
        item.assigned_profile
          ? [{
              agent_id: item.id,
              asset: { domain: "model" as const, profile_id: item.assigned_profile },
              desired: true,
              observed: true,
              status: "synced" as const,
              reason: null,
              affected_agent_ids: [item.id],
            }]
          : [],
      ),
    ],
    external: [],
  };
}

export function AgentView({
  state,
  skillsState,
  consumptionState,
  agentId,
  onOpenResource,
  onOpenModels,
  onOpenSkills,
}: AgentViewProps) {
  const { entries, agents, refreshAgents, rescan } = state;
  const { show: showToast } = useToast();
  const [editingAgent, setEditingAgent] = useState(false);
  const [pickerDomain, setPickerDomain] = useState<AssetRef["domain"] | null>(null);
  const [modelProfiles, setModelProfiles] = useState<ModelProfileView[]>([]);
  const [modelAgents, setModelAgents] = useState<ModelAgentView[]>([]);
  const [modelsLoading, setModelsLoading] = useState(true);
  const [modelsError, setModelsError] = useState<string | null>(null);
  const [resourceTab, setResourceTab] = useState<AgentResourceTab>("mcps");

  const navigateResource = useCallback((request: ResourceNavigationRequest) => {
    if (onOpenResource) return onOpenResource(request);
    if (request.domain === "skill") return onOpenSkills?.(request);
    if (request.domain === "model") return onOpenModels?.();
  }, [onOpenModels, onOpenResource, onOpenSkills]);

  const agent = useMemo(
    () => agents.find((item) => item.id === agentId) ?? null,
    [agents, agentId],
  );

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
      ? modelProfiles.filter((profile) => modelAgent.supported_protocols.includes(profile.protocol))
      : [],
    [modelAgent, modelProfiles],
  );
  const inventory = consumptionState?.inventory ?? fallbackInventory(state, modelAgents);
  const mcpRows = consumptionsForAgent(inventory, agentId, "mcp");
  const modelRows = consumptionsForAgent(inventory, agentId, "model");
  const skillRows = consumptionsForAgent(inventory, agentId, "skill");
  const mcpExternal = externalForAgent(inventory, agentId, "mcp");
  const modelExternal = externalForAgent(inventory, agentId, "model");
  const skillExternal = externalForAgent(inventory, agentId, "skill");

  if (!agent) return <div className="mux-agent-state">未找到该 Agent</div>;

  if (!agent.has_global) {
    return (
      <div className="mux-agent-page">
        <div className="mux-agent-shell">
          <AgentHeader agent={agent} tone="reference" />
          <div className="mux-agent-reference">
            <strong>{agent.note ?? "未提供可写的用户级全局配置。"}</strong>
            <span>
              {agent.category} · {agent.evidence === "community-extension" ? "社区扩展" : "公开来源"}
              {agent.verified_at ? ` · ${agent.verified_at}` : ""}
            </span>
          </div>
        </div>
      </div>
    );
  }

  const mcpConfigPath = agent.global ?? "";
  const skillsConfigPath = agent.skills_global_dir;
  const runtimeSkillAgent = skillsState.inventory?.agents.find((item) => item.id === agentId) ?? null;
  const modelRelationship = modelAgent
    ? samePath(modelAgent.config_path, mcpConfigPath) ? "Model / MCP 共用" : "Model / MCP 分离"
    : null;
  const modelDescription = modelsLoading
    ? "正在读取模型配置…"
    : modelsError
      ? `读取模型配置失败：${modelsError}`
      : modelAgent?.mode === "guided"
        ? "官方引导"
        : modelAgent ? "模型配置文件" : "尚未接入 Models";
  const skillsDescription = !skillsConfigPath
    ? "尚未核验 Skills 目录"
    : skillsState.loading
      ? "正在读取 Skills 状态…"
      : skillsState.error
        ? `读取 Skills 状态失败：${skillsState.error.message}`
        : !skillsState.inventory
          ? "已核验用户级目录"
          : !runtimeSkillAgent
            ? "已核验目录 · 未检测到 Agent"
            : runtimeSkillAgent.affected_agent_ids.length > 1
              ? `用户级目录 · 共享影响 ${runtimeSkillAgent.affected_agent_ids.length} 个 Agent`
              : "用户级目录 · 已检测";

  const centralSkills = (skillsState.inventory?.items ?? []).filter(
    (item) => item.location.kind === "central" && item.states.includes("managed"),
  );
  const picker = pickerDomain ? pickerData(pickerDomain) : null;

  function pickerData(domain: AssetRef["domain"]): {
    title: string;
    mode: "single" | "multiple";
    selectedIds: string[];
    options: ConsumptionPickerOption[];
  } {
    if (domain === "mcp") {
      return {
        title: "管理正在使用的 MCPs",
        mode: "multiple",
        selectedIds: mcpRows.flatMap((item) => item.asset.domain === "mcp" ? [item.asset.key] : []),
        options: entries
          .filter((entry) => agent?.supported_transports.includes(transportOf(entry)))
          .map((entry) => ({
            id: keyOf(entry),
            name: entry.name,
            description: entry.description,
            meta: <TransportPill entry={entry} compact />,
          })),
      };
    }
    if (domain === "model") {
      return {
        title: "切换正在使用的 Model",
        mode: "single",
        selectedIds: modelRows.flatMap((item) => item.asset.domain === "model" ? [item.asset.profile_id] : []),
        options: compatibleProfiles.map((profile) => ({
          id: profile.id,
          name: profile.name,
          description: `${profile.model} · ${profile.protocol}`,
          reason: profile.credential_saved ? undefined : "未保存 Keychain 凭据；仅适用于无鉴权端点。",
        })),
      };
    }
    return {
      title: "管理正在使用的 Skills",
      mode: "multiple",
      selectedIds: skillRows.flatMap((item) => item.asset.domain === "skill" ? [item.asset.name] : []),
      options: centralSkills.map((item) => ({
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

  const planSelection = async (domain: AssetRef["domain"], ids: string[]) => {
    if (!consumptionState) return;
    try {
      await consumptionState.planForAgent(agentId, createSelection(domain, ids));
      setPickerDomain(null);
    } catch (error) {
      showToast({ kind: "error", msg: "无法生成消费计划：" + formatError(error) });
    }
  };

  const commitPlan = async (conflictConfirmation?: string) => {
    if (!consumptionState) return;
    try {
      await consumptionState.commit(conflictConfirmation);
      await Promise.all([
        rescan().catch(() => undefined),
        skillsState.refresh().catch(() => undefined),
        refreshModels().catch(() => undefined),
      ]);
      showToast({ kind: "success", msg: "中央资产消费关系已同步。" });
    } catch (error) {
      showToast({ kind: "error", msg: "同步失败：" + formatError(error) });
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
        <AgentHeader agent={agent} />

        <section className="mux-agent-section" aria-labelledby="agent-files-title" aria-label="配置位置">
          <div className="mux-agent-section-head">
            <div>
              <h3 id="agent-files-title">配置位置</h3>
              <p>这些路径是消费中央资产后的落盘目标，不是资产安装入口。</p>
            </div>
            {modelRelationship && <Badge tone="info">{modelRelationship}</Badge>}
          </div>
          <div className="mux-agent-file-map">
            <ConfigPath
              icon={<PackageIcon className="w-4 h-4" />}
              label="MCPs"
              description={`${agent.key} · ${agent.format.toUpperCase()}`}
              path={mcpConfigPath}
              action={
                <IconButton title="编辑 MCP 配置文件路径" onClick={() => setEditingAgent(true)}>
                  <EditIcon className="w-4 h-4" />
                </IconButton>
              }
            />
            <ConfigPath icon={<LayersIcon className="w-4 h-4" />} label="Models" description={modelDescription} path={modelAgent?.config_path ?? null} />
            <ConfigPath icon={<SparklesIcon className="w-4 h-4" />} label="Skills" description={skillsDescription} path={skillsConfigPath} />
          </div>
        </section>

        <AgentResourcePanel
          value={resourceTab}
          onChange={setResourceTab}
          counts={{ mcps: mcpRows.length, models: modelRows.length, skills: skillRows.length }}
        >
          {resourceTab === "mcps" ? (
            <AgentConsumptionPanel
              title="正在使用"
              description="中央 MCP 资产与当前 Agent 的 desired relationship。"
              manageLabel="管理 MCPs"
              rows={mcpRows}
              external={mcpExternal}
              onManage={() => setPickerDomain("mcp")}
              manageDisabled={!consumptionState}
              onOpenAsset={openAsset}
              present={(asset) => {
                const key = asset.domain === "mcp" ? asset.key : "";
                const entry = entries.find((candidate) => keyOf(candidate) === key);
                return {
                  name: entry?.name ?? key.replace(/::(?:stdio|http)$/, ""),
                  description: entry?.description || key,
                  icon: <Avatar seed={entry?.name ?? key} size={30} />,
                  meta: entry ? <TransportPill entry={entry} compact /> : null,
                };
              }}
            />
          ) : resourceTab === "models" ? (
            modelAgent?.mode === "guided" ? (
              <section className="mux-agent-section mux-agent-resource-content">
                <div className="mux-agent-guided-model">
                  <div><strong>通过 Agent 官方流程配置</strong><span>{modelAgent.note}</span></div>
                  <button type="button" className="btn-secondary" onClick={() => openUrl(modelAgent.docs)}>
                    <LinkIcon className="w-4 h-4" />打开设置文档
                  </button>
                </div>
              </section>
            ) : (
              <AgentConsumptionPanel
                title="正在使用"
                description="每个 Agent 最多消费一个中央 Model Profile。"
                manageLabel="切换 Model"
                rows={modelRows}
                external={modelExternal}
                onManage={() => setPickerDomain("model")}
                onOpenAsset={openAsset}
                manageDisabled={!consumptionState || !modelAgent || modelsLoading}
                present={(asset) => {
                  const id = asset.domain === "model" ? asset.profile_id : "";
                  if (id.startsWith("external-")) {
                    return {
                      name: "外部 Model 配置",
                      description: "已在 Agent 配置中检测到，但尚未纳入中央 Model Profiles。",
                      icon: <LayersIcon className="w-4 h-4" />,
                      meta: <Badge tone="warning">只读</Badge>,
                    };
                  }
                  const profile = modelProfiles.find((candidate) => candidate.id === id);
                  return {
                    name: profile?.name ?? id,
                    description: profile ? `${profile.model} · ${profile.protocol}` : "中央 Profile 已缺失",
                    icon: <LayersIcon className="w-4 h-4" />,
                    meta: profile?.credential_saved ? <Badge tone="success">Keychain</Badge> : null,
                  };
                }}
              />
            )
          ) : (
            <AgentConsumptionPanel
              title="正在使用"
              description="只从中央 Skills 资产库建立关系；这里不再安装 Skill。"
              manageLabel="管理 Skills"
              rows={skillRows}
              external={skillExternal}
              onManage={() => setPickerDomain("skill")}
              onOpenAsset={openAsset}
              manageDisabled={!runtimeSkillAgent || !consumptionState}
              present={(asset) => {
                const name = asset.domain === "skill" ? asset.name : "";
                const skill = centralSkills.find((candidate) => candidate.name === name);
                return {
                  name,
                  description: skill?.description ?? "中央 Skill 已缺失",
                  icon: <SparklesIcon className="w-4 h-4" />,
                  meta: skillRows.find((row) => row.asset.domain === "skill" && row.asset.name === name)?.affected_agent_ids.length! > 1
                    ? <Badge tone="warning">共享目标</Badge>
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
          subtitle={`仅选择中央资产；确认后会先展示对 ${agent.name} 的完整影响。`}
          mode={picker.mode}
          options={picker.options}
          selectedIds={picker.selectedIds}
          onClose={() => setPickerDomain(null)}
          onReview={(ids) => planSelection(pickerDomain, ids)}
        />
      )}

      {consumptionState?.plan && (
        <AssetOperationReviewDialog
          plan={consumptionState.plan}
          busy={consumptionState.committing}
          error={consumptionState.error?.message}
          onCommit={commitPlan}
          onCancel={consumptionState.cancel}
        />
      )}

      {editingAgent && (
        <AddAgentDialog
          existing={agent}
          onClose={() => setEditingAgent(false)}
          onAdded={async () => {
            await refreshAgents();
            await rescan();
            await consumptionState?.refresh();
          }}
        />
      )}
    </div>
  );
}

function AgentHeader({ agent, tone }: { agent: InstallState["agents"][number]; tone?: "reference" }) {
  return (
    <header className="mux-agent-header">
      <AgentGlyph id={agent.id} name={agent.name} size={44} />
      <div className="mux-agent-header-copy">
        <div>
          <h2>{agent.name}</h2>
          {tone === "reference" ? <Badge>仅供参考</Badge> : agent.evidence === "community-extension" ? (
            <Badge tone="warning">社区扩展</Badge>
          ) : agent.builtin ? <Badge tone="success">已核验</Badge> : <Badge>自定义</Badge>}
        </div>
        <span>{agent.id} · {agent.category}</span>
      </div>
      {agent.docs && (
        <IconButton title="打开官方文档" onClick={() => openUrl(agent.docs!)}>
          <LinkIcon className="w-4 h-4" />
        </IconButton>
      )}
    </header>
  );
}

function ConfigPath({
  icon,
  label,
  description,
  path,
  action,
}: {
  icon: ReactNode;
  label: string;
  description: string;
  path: string | null;
  action?: ReactNode;
}) {
  return (
    <div className="mux-agent-file-row">
      <span className="mux-agent-file-icon">{icon}</span>
      <div className="mux-agent-file-copy">
        <div><strong>{label}</strong><span>{description}</span></div>
        {path ? <code title={path}>{path}</code> : <span className="mux-agent-file-unavailable">不可用</span>}
      </div>
      {action}
    </div>
  );
}
