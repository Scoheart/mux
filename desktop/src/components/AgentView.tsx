import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { InstallState } from "../hooks/useInstallState";
import type {
  ModelAgentView,
  ModelProfileView,
  ModelProtocol,
  RegistryEntry,
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
  TrashIcon,
} from "./icons";
import { Avatar, Badge, IconButton, SearchBar, Switch, TransportPill } from "./ui";
import { AgentGlyph } from "./brandIcons";
import { AddAgentDialog } from "./AddAgentDialog";
import { AgentPicker } from "./AgentPicker";
import { FeatureShell } from "./FeatureShell";
import { useToast } from "./Toast";

interface AgentViewProps {
  state: InstallState;
  agentId: string;
  onSelectAgent: (id: string) => void;
  onAddAgent?: () => void;
  onSelectMcps: () => void;
  onSelectModels: () => void;
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
  agentId,
  onSelectAgent,
  onAddAgent,
  onSelectMcps,
  onSelectModels,
}: AgentViewProps) {
  const onOpenModels = onSelectModels;
  const { entries, agents, installed, pending, toggle, setEnabled, remove, refreshAgents, rescan } = state;
  const { show: showToast } = useToast();

  const [showAddPopover, setShowAddPopover] = useState(false);
  const [addSearch, setAddSearch] = useState("");
  const [editingAgent, setEditingAgent] = useState(false);
  const [modelProfiles, setModelProfiles] = useState<ModelProfileView[]>([]);
  const [modelAgents, setModelAgents] = useState<ModelAgentView[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState("");
  const [modelsLoading, setModelsLoading] = useState(true);
  const [applyingModel, setApplyingModel] = useState(false);

  const agent = useMemo(() => agents.find((item) => item.id === agentId) ?? null, [agents, agentId]);

  const refreshModels = useCallback(async () => {
    const [profiles, nextAgents] = await Promise.all([listModelProfiles(), listModelAgents()]);
    setModelProfiles(profiles);
    setModelAgents(nextAgents);
  }, []);

  useEffect(() => {
    setModelsLoading(true);
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
    const search = addSearch.trim().toLowerCase();
    return entries
      .filter((entry) => {
        if (!agent?.supported_transports.includes(transportOf(entry))) return false;
        if (installedKeySet.has(keyOf(entry))) return false;
        return !search || entry.name.toLowerCase().includes(search) || entry.description.toLowerCase().includes(search);
      })
      .sort(
        (left, right) =>
          left.name.localeCompare(right.name, undefined, { sensitivity: "base" }) ||
          transportLabel(left).localeCompare(transportLabel(right))
      );
  }, [entries, installedKeySet, addSearch, agent]);

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
  const agentConfigPath = modelAgent?.config_path || mcpConfigPath;
  const sharedConfig = samePath(agentConfigPath, mcpConfigPath);

  return (
    <FeatureShell
      active="mcps"
      onSelectMcps={onSelectMcps}
      onSelectModels={onSelectModels}
      toolbar={
        <div className="mux-feature-chrome-toolbar">
          <AgentPicker
            agents={agents}
            selectedId={agentId}
            onSelect={onSelectAgent}
            onAddAgent={onAddAgent}
          />
        </div>
      }
    >
      <div className="mux-agent-page">
      <div className="mux-agent-shell">
        <AgentHeader agent={agent} />

        <section className="mux-agent-section" aria-labelledby="agent-files-title">
          <div className="mux-agent-section-head">
            <div>
              <h3 id="agent-files-title">配置文件</h3>
              <p>明确模型设置与 MCP 管理使用的全局文件。</p>
            </div>
            <Badge tone={sharedConfig ? "info" : "neutral"}>
              {sharedConfig ? "同一文件" : "独立 MCP 文件"}
            </Badge>
          </div>
          <div className="mux-agent-file-map">
            <ConfigPath
              icon={<LayersIcon className="w-4 h-4" />}
              label="Agent 配置文件"
              description={modelAgent ? "模型与运行设置" : "当前已核验的全局配置"}
              path={agentConfigPath}
            />
            <ConfigPath
              icon={<PackageIcon className="w-4 h-4" />}
              label="MCP 配置文件"
              description={`${agent.key} · ${agent.format.toUpperCase()}`}
              path={mcpConfigPath}
              action={
                <IconButton title="编辑 MCP 配置文件路径" onClick={() => setEditingAgent(true)}>
                  <EditIcon className="w-4 h-4" />
                </IconButton>
              }
            />
          </div>
        </section>

        <section className="mux-agent-section" aria-labelledby="agent-model-title">
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
            onOpenModels={onOpenModels}
          />
        </section>

        <section className="mux-agent-section" aria-labelledby="agent-mcp-title">
          <div className="mux-agent-section-head mux-agent-mcp-head">
            <div>
              <h3 id="agent-mcp-title">MCP</h3>
              <p>{installedEntries.length} 个已添加，开关会同步更新 MCP 配置区。</p>
            </div>
            <div className="mux-agent-add-wrap">
              <button
                type="button"
                className="btn-primary"
                onClick={() => {
                  setShowAddPopover((value) => !value);
                  setAddSearch("");
                }}
              >
                <PlusIcon className="w-3.5 h-3.5" />
                添加 MCP
              </button>
              {showAddPopover && (
                <>
                  <div
                    className="mux-agent-popover-scrim"
                    onClick={() => {
                      setShowAddPopover(false);
                      setAddSearch("");
                    }}
                  />
                  <div className="mux-agent-add-popover" onClick={(event) => event.stopPropagation()}>
                    <div className="mux-agent-add-search">
                      <SearchBar value={addSearch} onChange={setAddSearch} placeholder="搜索 MCP…" autoFocus />
                    </div>
                    <div className="mux-agent-add-list">
                      {notInstalledEntries.length === 0 ? (
                        <div className="mux-agent-add-empty">
                          {entries.length === installedEntries.length ? "所有 MCP 均已添加" : "未找到匹配的 MCP"}
                        </div>
                      ) : (
                        notInstalledEntries.map((entry) => {
                          const isPending = pending.has(cellKey(keyOf(entry), agentId));
                          return (
                            <button
                              key={keyOf(entry)}
                              type="button"
                              className="mux-agent-add-item"
                              disabled={isPending}
                              onClick={() => {
                                handleToggle(entry);
                                setShowAddPopover(false);
                                setAddSearch("");
                              }}
                            >
                              <Avatar seed={entry.name} size={30} />
                              <span className="mux-agent-add-copy">
                                <span>
                                  <strong>{entry.name}</strong>
                                  <TransportPill entry={entry} compact />
                                </span>
                                {entry.description && <small>{entry.description}</small>}
                              </span>
                            </button>
                          );
                        })
                      )}
                    </div>
                  </div>
                </>
              )}
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
        </section>
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
    </FeatureShell>
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
  path: string;
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
        <code title={path}>{path}</code>
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
          <strong>通过 Agent 内置流程配置</strong>
          <span>该 Agent 暂未公开安全的非交互模型写入接口，请通过官方设置流程完成配置。</span>
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
