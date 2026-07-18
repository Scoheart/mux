import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { InstallState } from "../hooks/useInstallState";
import type { SkillsState } from "../hooks/useSkillsState";
import type {
  ModelAgentView,
  ModelProfileView,
  ModelProtocol,
  RegistryEntry,
  ResourceNavigationRequest,
} from "../lib/types";
import { formatError } from "../lib/format";
import { keyOf, transportLabel, installedKey, transportOf } from "../lib/mcp";
import {
  applyModelProfile,
  cellKey,
  listModelAgents,
  listModelProfiles,
} from "../lib/api";
import {
  CheckIcon,
  EditIcon,
  LayersIcon,
  LinkIcon,
  PackageIcon,
  PlusIcon,
  SparklesIcon,
  TrashIcon,
} from "./icons";
import { Avatar, Badge, IconButton, Switch, TransportPill } from "./ui";
import { AgentGlyph } from "./brandIcons";
import { AddAgentDialog } from "./AddAgentDialog";
import { AgentSkillsSection } from "./AgentSkillsSection";
import { useToast } from "./Toast";
import { AgentResourcePanel, type AgentResourceTab } from "./AgentResourcePanel";
import { ResourcePickerDialog } from "./ResourcePickerDialog";

interface AgentViewProps {
  state: InstallState;
  skillsState: SkillsState;
  agentId: string;
  onOpenResource?(request: ResourceNavigationRequest): void;
  /** Transitional test adapter; production uses onOpenResource. */
  onOpenModels?: () => void;
  /** Transitional test adapter; production uses onOpenResource. */
  onOpenSkills?: (request: Extract<ResourceNavigationRequest, { domain: "skill" }>) => void;
}

function syntheticEntry(serverKey: string): RegistryEntry {
  const idx = serverKey.lastIndexOf("::");
  const name = idx >= 0 ? serverKey.slice(0, idx) : serverKey;
  const transport = idx >= 0 ? serverKey.slice(idx + 2) : "stdio";
  return {
    name,
    description: "",
    tags: [],
    config: transport === "http" ? { http: { type: "http", url: "" } } : { stdio: { command: "" } },
  };
}

function protocolLabel(protocol: ModelProtocol) {
  if (protocol === "anthropic-messages") return "Anthropic Messages";
  if (protocol === "openai-responses") return "OpenAI Responses";
  return "OpenAI Completions";
}

function samePath(left: string, right: string) {
  return left.trim().replace(/\/+$/, "") === right.trim().replace(/\/+$/, "");
}

export function AgentView({
  state,
  skillsState,
  agentId,
  onOpenResource,
  onOpenModels,
  onOpenSkills,
}: AgentViewProps) {
  const { entries, agents, installed, pending, toggle, setEnabled, remove, refreshAgents, rescan } = state;
  const { show: showToast } = useToast();

  const [mcpPickerOpen, setMcpPickerOpen] = useState(false);
  const [editingAgent, setEditingAgent] = useState(false);
  const [modelProfiles, setModelProfiles] = useState<ModelProfileView[]>([]);
  const [modelAgents, setModelAgents] = useState<ModelAgentView[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState("");
  const [modelsLoading, setModelsLoading] = useState(true);
  const [modelsError, setModelsError] = useState<string | null>(null);
  const [applyingModel, setApplyingModel] = useState(false);
  const [resourceTab, setResourceTab] = useState<AgentResourceTab>("mcps");
  const navigateResource = useCallback((request: ResourceNavigationRequest) => {
    if (onOpenResource) return onOpenResource(request);
    if (request.domain === "skill") return onOpenSkills?.(request);
    if (request.domain === "model") return onOpenModels?.();
  }, [onOpenModels, onOpenResource, onOpenSkills]);

  const agent = useMemo(() => agents.find((item) => item.id === agentId) ?? null, [agents, agentId]);

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
      .catch((error) =>
        showToast({ kind: "error", msg: "读取模型配置失败：" + formatError(error) })
      )
      .finally(() => setModelsLoading(false));
  }, [refreshModels, showToast]);

  const modelAgent = useMemo(
    () => modelAgents.find((item) => item.id === agentId) ?? null,
    [modelAgents, agentId]
  );

  const compatibleProfiles = useMemo(
    () =>
      modelAgent
        ? modelProfiles.filter((profile) => modelAgent.supported_protocols.includes(profile.protocol))
        : [],
    [modelAgent, modelProfiles]
  );

  useEffect(() => {
    setSelectedProfileId((current) => {
      if (compatibleProfiles.some((profile) => profile.id === current)) return current;
      if (
        modelAgent?.assigned_profile &&
        compatibleProfiles.some((profile) => profile.id === modelAgent.assigned_profile)
      ) {
        return modelAgent.assigned_profile;
      }
      return compatibleProfiles[0]?.id ?? "";
    });
  }, [compatibleProfiles, modelAgent]);

  const currentProfile = useMemo(
    () => modelProfiles.find((profile) => profile.id === modelAgent?.assigned_profile) ?? null,
    [modelAgent, modelProfiles]
  );
  const selectedProfile = useMemo(
    () => modelProfiles.find((profile) => profile.id === selectedProfileId) ?? null,
    [modelProfiles, selectedProfileId]
  );

  const agentRows = useMemo(
    () => installed.filter((item) => item.agent === agentId && item.scope === "global"),
    [installed, agentId]
  );

  const installedKeySet = useMemo(
    () => new Set(agentRows.map((row) => installedKey(row))),
    [agentRows]
  );

  const installedEntries = useMemo(
    () =>
      agentRows
        .map((row) => {
          const key = installedKey(row);
          const entry = entries.find((item) => keyOf(item) === key) ?? syntheticEntry(key);
          return { entry, enabled: row.enabled };
        })
        .sort(
          (left, right) =>
            left.entry.name.localeCompare(right.entry.name, undefined, { sensitivity: "base" }) ||
            transportLabel(left.entry).localeCompare(transportLabel(right.entry))
        ),
    [agentRows, entries]
  );

  const notInstalledEntries = useMemo(() => {
    return entries
      .filter((entry) => {
        if (!agent?.supported_transports.includes(transportOf(entry))) return false;
        if (installedKeySet.has(keyOf(entry))) return false;
        return true;
      })
      .sort(
        (left, right) =>
          left.name.localeCompare(right.name, undefined, { sensitivity: "base" }) ||
          transportLabel(left).localeCompare(transportLabel(right))
      );
  }, [entries, installedKeySet, agent]);

  const handleToggle = useCallback(
    (entry: RegistryEntry) => {
      const key = cellKey(keyOf(entry), agentId);
      if (!pending.has(key)) toggle(entry, agentId);
    },
    [agentId, pending, toggle]
  );

  const handleApplyModel = async () => {
    if (!modelAgent || modelAgent.mode !== "managed" || !selectedProfile) return;
    setApplyingModel(true);
    try {
      const result = await applyModelProfile(modelAgent.id, selectedProfile.id);
      await refreshModels();
      showToast({ kind: "success", msg: result.message || `已将 ${selectedProfile.name} 应用到 ${agent?.name ?? modelAgent.name}` });
    } catch (error) {
      showToast({ kind: "error", msg: "应用模型失败：" + formatError(error) });
    } finally {
      setApplyingModel(false);
    }
  };

  if (!agent) {
    return <div className="mux-agent-state">未找到该 Agent</div>;
  }

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
  const runtimeSkillAgent = skillsState.inventory?.agents.find(
    (item) => item.id === agentId,
  ) ?? null;
  const modelRelationship = modelAgent
    ? samePath(modelAgent.config_path, mcpConfigPath)
      ? "Model / MCP 共用"
      : "Model / MCP 分离"
    : null;
  const modelDescription = modelsLoading
    ? "正在读取模型配置…"
    : modelsError
      ? `读取模型配置失败：${modelsError}`
      : modelAgent?.mode === "guided"
        ? "官方引导"
        : modelAgent
          ? "模型配置文件"
          : "尚未接入 Models";
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
  const skillTargetIds = new Set(
    skillsState.inventory?.targets
      .filter((target) => target.affected_agent_ids.includes(agentId))
      .map((target) => target.target_id) ?? [],
  );
  const assignedSkillCount = skillsState.inventory?.items.filter(
    (item) =>
      item.location.kind === "central" &&
      item.assigned_target_ids.some((targetId) => skillTargetIds.has(targetId)),
  ).length ?? 0;

  return (
    <div className="mux-agent-page">
      <div className="mux-agent-shell">
        <AgentHeader agent={agent} />

        <section
          className="mux-agent-section"
          aria-labelledby="agent-files-title"
          aria-label="配置位置"
        >
          <div className="mux-agent-section-head">
            <div>
              <h3 id="agent-files-title">配置位置</h3>
              <p>MCP、Model 与 Skills 使用的用户级配置入口。</p>
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
            <ConfigPath
              icon={<LayersIcon className="w-4 h-4" />}
              label="Models"
              description={modelDescription}
              path={modelAgent?.config_path ?? null}
            />
            <ConfigPath
              icon={<SparklesIcon className="w-4 h-4" />}
              label="Skills"
              description={skillsDescription}
              path={skillsConfigPath}
            />
          </div>
        </section>

        <AgentResourcePanel
          value={resourceTab}
          onChange={setResourceTab}
          counts={{ mcps: installedEntries.length, models: compatibleProfiles.length, skills: assignedSkillCount }}
        >
        {resourceTab === "models" ? (
        <section className="mux-agent-section mux-agent-resource-content" aria-labelledby="agent-model-title">
          <div className="mux-agent-section-head">
            <div>
              <h3 id="agent-model-title">Model</h3>
              <p>选择兼容配置并直接应用到当前 Agent。</p>
            </div>
            <Badge tone="info">Beta</Badge>
          </div>
          <ModelAssignment
            loading={modelsLoading}
            agent={modelAgent}
            currentProfile={currentProfile}
            selectedProfile={selectedProfile}
            compatibleProfiles={compatibleProfiles}
            selectedProfileId={selectedProfileId}
            applying={applyingModel}
            onSelect={setSelectedProfileId}
            onApply={() => void handleApplyModel()}
            onOpenModels={() => navigateResource({ domain: "model", kind: "create" })}
            onOpenModelDetail={(profileId) => navigateResource({ domain: "model", kind: "detail", profileId })}
          />
        </section>
        ) : resourceTab === "skills" ? (
        <AgentSkillsSection
          key={agentId}
          agentId={agentId}
          state={skillsState}
          onOpenSkills={navigateResource}
        />
        ) : (
        <section className="mux-agent-section mux-agent-resource-content" aria-labelledby="agent-mcp-title">
          <div className="mux-agent-section-head mux-agent-mcp-head">
            <div>
              <h3 id="agent-mcp-title">MCP</h3>
              <p>{installedEntries.length} 个已添加，开关会同步更新 MCP 配置区。</p>
            </div>
            <div className="mux-agent-add-wrap">
              <button
                type="button"
                className="btn-primary"
                onClick={() => setMcpPickerOpen(true)}
              >
                <PlusIcon className="w-3.5 h-3.5" />
                添加 MCP
              </button>
            </div>
          </div>

          {installedEntries.length === 0 ? (
            <div className="mux-agent-mcp-empty">
              <PackageIcon className="w-7 h-7" />
              <strong>还没有添加 MCP</strong>
              <span>从 MUX 资源库选择后会写入上方标明的 MCP 文件。</span>
            </div>
          ) : (
            <div className="mux-agent-mcp-grid">
              {installedEntries.map(({ entry, enabled }) => {
                const isPending = pending.has(cellKey(keyOf(entry), agentId));
                return (
                  <div
                    key={keyOf(entry)}
                    className="mux-agent-mcp-row"
                    data-enabled={enabled ? "true" : "false"}
                    data-pending={isPending ? "true" : undefined}
                  >
                    <Avatar seed={entry.name} size={30} />
                    <span className="mux-agent-mcp-name">
                      <strong title={entry.name}>{entry.name}</strong>
                      <TransportPill entry={entry} compact />
                    </span>
                    <Switch
                      checked={enabled}
                      disabled={isPending}
                      title={enabled ? "禁用 MCP" : "启用 MCP"}
                      onChange={(nextEnabled) => {
                        if (!isPending) setEnabled(entry, agentId, nextEnabled);
                      }}
                    />
                    <IconButton
                      title={`查看 ${entry.name} 详情`}
                      disabled={isPending}
                      onClick={() => navigateResource({
                        domain: "mcp",
                        kind: "detail",
                        name: entry.name,
                        transport: transportOf(entry),
                      })}
                    >
                      <LinkIcon className="w-4 h-4" />
                    </IconButton>
                    <IconButton
                      title="删除 MCP"
                      disabled={isPending}
                      onClick={() => {
                        if (!isPending) remove(entry, agentId);
                      }}
                    >
                      <TrashIcon className="w-4 h-4" />
                    </IconButton>
                  </div>
                );
              })}
            </div>
          )}
          {mcpPickerOpen && (
            <ResourcePickerDialog
              title="添加 MCP"
              subtitle={`选择要添加到 ${agent.name} 的 MCP。`}
              options={notInstalledEntries.map((entry) => ({
                id: keyOf(entry),
                name: entry.name,
                description: entry.description,
                avatar: <Avatar seed={entry.name} size={30} />,
                meta: <TransportPill entry={entry} compact />,
                disabled: pending.has(cellKey(keyOf(entry), agentId)),
              }))}
              addLabel="添加 MCP"
              onClose={() => setMcpPickerOpen(false)}
              onAdd={(option) => {
                const entry = notInstalledEntries.find((candidate) => keyOf(candidate) === option.id);
                if (!entry) return;
                handleToggle(entry);
                setMcpPickerOpen(false);
              }}
            />
          )}
        </section>
        )}
        </AgentResourcePanel>
      </div>

      {editingAgent && (
        <AddAgentDialog
          existing={agent}
          onClose={() => setEditingAgent(false)}
          onAdded={async () => {
            await refreshAgents();
            await rescan();
          }}
        />
      )}
    </div>
  );
}

function AgentHeader({
  agent,
  tone,
}: {
  agent: InstallState["agents"][number];
  tone?: "reference";
}) {
  return (
    <header className="mux-agent-header">
      <AgentGlyph id={agent.id} name={agent.name} size={44} />
      <div className="mux-agent-header-copy">
        <div>
          <h2>{agent.name}</h2>
          {tone === "reference" ? (
            <Badge>仅供参考</Badge>
          ) : agent.evidence === "community-extension" ? (
            <Badge tone="warning">社区扩展</Badge>
          ) : agent.builtin ? (
            <Badge tone="success">已核验</Badge>
          ) : (
            <Badge>自定义</Badge>
          )}
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
        <div>
          <strong>{label}</strong>
          <span>{description}</span>
        </div>
        {path ? (
          <code title={path}>{path}</code>
        ) : (
          <span className="mux-agent-file-unavailable">不可用</span>
        )}
      </div>
      {action}
    </div>
  );
}

function ModelAssignment({
  loading,
  agent,
  currentProfile,
  selectedProfile,
  compatibleProfiles,
  selectedProfileId,
  applying,
  onSelect,
  onApply,
  onOpenModels,
  onOpenModelDetail,
}: {
  loading: boolean;
  agent: ModelAgentView | null;
  currentProfile: ModelProfileView | null;
  selectedProfile: ModelProfileView | null;
  compatibleProfiles: ModelProfileView[];
  selectedProfileId: string;
  applying: boolean;
  onSelect: (profileId: string) => void;
  onApply: () => void;
  onOpenModels: () => void;
  onOpenModelDetail: (profileId: string) => void;
}) {
  if (loading) return <div className="mux-agent-inline-state">读取模型配置…</div>;

  if (!agent) {
    return (
      <div className="mux-agent-inline-state">
        <span>Models Beta 尚未接入此 Agent，MCP 管理不受影响。</span>
      </div>
    );
  }

  if (agent.mode === "guided") {
    return (
      <div className="mux-agent-guided-model">
        <div>
          <strong>通过 Agent 官方流程配置</strong>
          <span>{agent.note}</span>
        </div>
        <button type="button" className="btn-secondary" onClick={() => openUrl(agent.docs)}>
          <LinkIcon className="w-4 h-4" />
          打开设置文档
        </button>
      </div>
    );
  }

  if (compatibleProfiles.length === 0) {
    return (
      <div className="mux-agent-inline-state mux-agent-inline-state-action">
        <span>没有兼容的模型配置，先在 Models 中创建。</span>
        <button type="button" className="btn-secondary" onClick={onOpenModels}>
          <PlusIcon className="w-4 h-4" />
          新建模型
        </button>
      </div>
    );
  }

  const alreadyApplied = currentProfile?.id === selectedProfile?.id;
  return (
    <div className="mux-agent-model-control">
      <div className="mux-agent-model-current">
        <AgentGlyph id={agent.id} name={agent.name} size={34} />
        <div>
          <span>当前模型</span>
          <strong>{currentProfile?.name ?? "未配置"}</strong>
          <code>{currentProfile?.model ?? "尚未应用模型配置"}</code>
        </div>
        {!agent.installed && <Badge tone="warning">未检测到应用</Badge>}
        {currentProfile && (
          <IconButton title={`查看 ${currentProfile.name} 详情`} onClick={() => onOpenModelDetail(currentProfile.id)}>
            <LinkIcon className="w-4 h-4" />
          </IconButton>
        )}
      </div>
      <div className="mux-agent-model-apply">
        <label htmlFor={`model-profile-${agent.id}`}>应用模型</label>
        <select
          id={`model-profile-${agent.id}`}
          className="mux-model-field"
          value={selectedProfileId}
          onChange={(event) => onSelect(event.target.value)}
        >
          {compatibleProfiles.map((profile) => (
            <option key={profile.id} value={profile.id}>
              {profile.name} · {profile.model}
            </option>
          ))}
        </select>
        <div className="mux-agent-model-preview">
          <span className="mux-model-protocol-dot" data-protocol={selectedProfile?.protocol} />
          <span>{selectedProfile ? protocolLabel(selectedProfile.protocol) : ""}</span>
          {selectedProfile?.credential_saved && (
            <span className="mux-agent-model-key"><CheckIcon className="w-3 h-3" /> Keychain</span>
          )}
        </div>
        <button
          type="button"
          className={alreadyApplied ? "btn-secondary" : "btn-primary"}
          disabled={!selectedProfile || applying || alreadyApplied}
          onClick={onApply}
        >
          {alreadyApplied ? <CheckIcon className="w-4 h-4" /> : <LayersIcon className="w-4 h-4" />}
          {applying ? "应用中…" : alreadyApplied ? "已应用" : "应用模型"}
        </button>
      </div>
    </div>
  );
}
