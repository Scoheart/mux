import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  applyModelProfile,
  deleteModelProfile,
  listModelAgents,
  listModelProfiles,
  saveModelProfile,
} from "../lib/api";
import type {
  ModelAgentView,
  ModelProfile,
  ModelProfileView,
  ModelProtocol,
  ResourceNavigationIntent,
} from "../lib/types";
import { formatError } from "../lib/format";
import { AgentGlyph } from "./brandIcons";
import { Avatar, Badge } from "./ui";
import { ResourceCard } from "./ResourceCard";
import { ResourceState } from "./ResourceState";
import { DialogShell } from "./DialogShell";
import { ReviewDialog } from "./ReviewDialog";
import {
  CheckIcon,
  EditIcon,
  LayersIcon,
  LinkIcon,
  PlusIcon,
  TrashIcon,
} from "./icons";
import { useToast } from "./Toast";
import {
  AgentStack,
  InspectorField,
  InspectorSection,
  ResourceGrid,
  ResourceInspector,
  ResourceTabs,
  ResourceWorkspace,
  SidebarItem,
  SidebarSection,
  WorkspaceSidebar,
} from "./ResourceWorkspace";

const PROTOCOLS: Array<{ id: ModelProtocol; label: string }> = [
  { id: "anthropic-messages", label: "Anthropic Messages" },
  { id: "openai-responses", label: "OpenAI Responses" },
  { id: "openai-completions", label: "OpenAI Completions" },
];

type ModelStatusFilter = "all" | "assigned" | "unassigned";
type ModelReview =
  | { kind: "delete"; profile: ModelProfileView }
  | { kind: "apply"; profile: ModelProfileView; agent: ModelAgentView };

const emptyProfile = (): ModelProfile => ({
  id: "",
  name: "",
  protocol: "openai-responses",
  base_url: "",
  model: "",
  reasoning: false,
});

function protocolLabel(protocol: ModelProtocol) {
  return PROTOCOLS.find((item) => item.id === protocol)?.label ?? protocol;
}

export function ModelsView({
  intent,
  onIntentConsumed,
}: {
  intent?: Extract<ResourceNavigationIntent, { domain: "model" }>;
  onIntentConsumed?(id: number): void;
} = {}) {
  const [profiles, setProfiles] = useState<ModelProfileView[]>([]);
  const [agents, setAgents] = useState<ModelAgentView[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [editing, setEditing] = useState<ModelProfileView | null | undefined>(undefined);
  const [busyAgent, setBusyAgent] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [readError, setReadError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<ModelStatusFilter>("all");
  const [protocolFilter, setProtocolFilter] = useState<ModelProtocol | null>(null);
  const [review, setReview] = useState<ModelReview | null>(null);
  const toast = useToast();
  const lastConsumedIntentId = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    const [nextProfiles, nextAgents] = await Promise.all([
      listModelProfiles(),
      listModelAgents(),
    ]);
    setProfiles(nextProfiles);
    setAgents(nextAgents);
    setSelectedProfileId((current) =>
      current && nextProfiles.some((profile) => profile.id === current) ? current : null
    );
  }, []);

  useEffect(() => {
    refresh()
      .then(() => setReadError(null))
      .catch((error) => {
        const message = formatError(error);
        setReadError(message);
        toast.show({ kind: "error", msg: "读取模型配置失败：" + message });
      })
      .finally(() => setLoading(false));
  }, [refresh]);

  const agentsByProfile = useMemo(() => {
    const map = new Map<string, ModelAgentView[]>();
    for (const agent of agents) {
      if (!agent.assigned_profile) continue;
      const rows = map.get(agent.assigned_profile) ?? [];
      rows.push(agent);
      map.set(agent.assigned_profile, rows);
    }
    return map;
  }, [agents]);

  const statusCounts = useMemo(() => {
    const scopedProfiles = protocolFilter
      ? profiles.filter((profile) => profile.protocol === protocolFilter)
      : profiles;
    const assigned = scopedProfiles.filter((profile) => (agentsByProfile.get(profile.id)?.length ?? 0) > 0).length;
    return { all: scopedProfiles.length, assigned, unassigned: scopedProfiles.length - assigned };
  }, [agentsByProfile, profiles, protocolFilter]);

  const protocolCounts = useMemo(
    () =>
      Object.fromEntries(
        PROTOCOLS.map((protocol) => [
          protocol.id,
          profiles.filter((profile) => profile.protocol === protocol.id).length,
        ])
      ) as Record<ModelProtocol, number>,
    [profiles]
  );

  const filteredProfiles = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    return profiles.filter((profile) => {
      const assigned = (agentsByProfile.get(profile.id)?.length ?? 0) > 0;
      if (statusFilter === "assigned" && !assigned) return false;
      if (statusFilter === "unassigned" && assigned) return false;
      if (protocolFilter && profile.protocol !== protocolFilter) return false;
      if (!needle) return true;
      return [profile.name, profile.id, profile.model, profile.base_url, protocolLabel(profile.protocol)]
        .join(" ")
        .toLocaleLowerCase()
        .includes(needle);
    });
  }, [agentsByProfile, profiles, protocolFilter, query, statusFilter]);

  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId) ?? null;

  useEffect(() => {
    if (!intent || loading || lastConsumedIntentId.current === intent.id) return;
    lastConsumedIntentId.current = intent.id;
    if (intent.kind === "create") {
      setEditing(null);
      onIntentConsumed?.(intent.id);
      return;
    }
    const profile = profiles.find((candidate) => candidate.id === intent.profileId);
    setQuery("");
    setProtocolFilter(null);
    setStatusFilter("all");
    setSelectedProfileId(profile?.id ?? null);
    if (!profile) toast.show({ kind: "error", msg: `未找到模型“${intent.profileId}”。` });
    onIntentConsumed?.(intent.id);
  }, [intent, loading, onIntentConsumed, profiles, toast]);

  const changeQuery = (value: string) => {
    setSelectedProfileId(null);
    setQuery(value);
  };

  const changeStatus = (status: ModelStatusFilter) => {
    setSelectedProfileId(null);
    setStatusFilter(status);
  };

  const changeProtocol = (protocol: ModelProtocol | null) => {
    setSelectedProfileId(null);
    setProtocolFilter(protocol);
  };

  const removeProfile = async (profile: ModelProfileView) => {
    try {
      await deleteModelProfile(profile.id);
      setSelectedProfileId((current) => current === profile.id ? null : current);
      await refresh();
      setReview(null);
      toast.show({ kind: "success", msg: `已删除：${profile.name}` });
    } catch (error) {
      toast.show({ kind: "error", msg: "删除失败：" + formatError(error) });
      throw error;
    }
  };

  const apply = async (agent: ModelAgentView, profile: ModelProfileView) => {
    setBusyAgent(agent.id);
    try {
      const result = await applyModelProfile(agent.id, profile.id);
      await refresh();
      setReview(null);
      toast.show({ kind: "success", msg: result.message });
    } catch (error) {
      toast.show({ kind: "error", msg: "应用失败：" + formatError(error) });
      throw error;
    } finally {
      setBusyAgent(null);
    }
  };

  return (
    <>
      <ResourceWorkspace
        sidebar={
          <WorkspaceSidebar title="Models" count={profiles.length}>
            <SidebarSection title="协议">
              <SidebarItem
                active={protocolFilter === null}
                icon={<LayersIcon className="w-3.5 h-3.5" />}
                label="全部协议"
                count={profiles.length}
                onClick={() => changeProtocol(null)}
              />
              {PROTOCOLS.map((protocol) => (
                <SidebarItem
                  key={protocol.id}
                  active={protocolFilter === protocol.id}
                  icon={<LayersIcon className="w-3.5 h-3.5" />}
                  label={protocol.label}
                  count={protocolCounts[protocol.id]}
                  onClick={() => changeProtocol(protocol.id)}
                />
              ))}
            </SidebarSection>
          </WorkspaceSidebar>
        }
        query={query}
        onQueryChange={changeQuery}
        searchPlaceholder="搜索模型"
        filters={
          <ResourceTabs
            label="模型状态"
            value={statusFilter}
            options={[
              { value: "all", label: "全部", count: statusCounts.all },
              { value: "assigned", label: "已分配", count: statusCounts.assigned },
              { value: "unassigned", label: "未分配", count: statusCounts.unassigned },
            ]}
            onChange={changeStatus}
          />
        }
        toolbarActions={
          <button className="btn-primary" type="button" onClick={() => setEditing(null)}>
            <PlusIcon className="w-4 h-4" />
            新建模型
          </button>
        }
        inspector={
          selectedProfile ? (
            <ModelInspector
              profile={selectedProfile}
              profiles={profiles}
              agents={agents}
              busyAgent={busyAgent}
              onApply={(agent, profile) => setReview({ kind: "apply", agent, profile })}
              onClose={() => setSelectedProfileId(null)}
              onEdit={() => setEditing(selectedProfile)}
              onDelete={() => setReview({ kind: "delete", profile: selectedProfile })}
            />
          ) : undefined
        }
        onInspectorClose={() => setSelectedProfileId(null)}
      >
        {loading ? (
          <ResourceState kind="loading" title="正在读取模型配置" />
        ) : readError ? (
          <ResourceState
            kind="read-error"
            icon={<LayersIcon className="w-6 h-6" />}
            title="读取模型配置失败"
            detail={readError}
            action={<button className="btn-primary" type="button" onClick={() => {
              setLoading(true);
              setReadError(null);
              void refresh()
                .then(() => setReadError(null))
                .catch((error) => setReadError(formatError(error)))
                .finally(() => setLoading(false));
            }}>重试</button>}
          />
        ) : filteredProfiles.length === 0 ? (
          <ResourceState
            kind={profiles.length === 0 ? "empty" : "no-match"}
            icon={<LayersIcon className="w-6 h-6" />}
            title={profiles.length === 0 ? "暂无模型" : "没有匹配项"}
            detail={profiles.length === 0 ? "创建模型后即可分配给 Agent" : "调整搜索、协议或状态筛选后重试。"}
            action={
              profiles.length === 0 ? undefined : (
                <button className="btn-secondary" type="button" onClick={() => {
                  setQuery("");
                  setProtocolFilter(null);
                  setStatusFilter("all");
                }}>清除筛选</button>
              )
            }
          />
        ) : (
          <ResourceGrid>
            {filteredProfiles.map((profile) => (
              <ModelCard
                key={profile.id}
                profile={profile}
                selected={profile.id === selectedProfileId}
                agentIds={(agentsByProfile.get(profile.id) ?? []).map((agent) => agent.id)}
                onOpen={() => setSelectedProfileId(profile.id)}
              />
            ))}
          </ResourceGrid>
        )}
      </ResourceWorkspace>
      {editing !== undefined && (
        <ModelProfileDialog
          initial={editing}
          onClose={() => setEditing(undefined)}
          onSaved={async () => {
            setEditing(undefined);
            await refresh();
          }}
        />
      )}
      {review?.kind === "delete" && (
        <ReviewDialog
          title="删除模型"
          subtitle={review.profile.name}
          confirmLabel="删除模型"
          onClose={() => setReview(null)}
          onConfirm={() => removeProfile(review.profile)}
        >
          <p>将删除此 MUX 模型配置与 Keychain 凭据引用；已经写入 Agent 的配置不会自动回滚。</p>
        </ReviewDialog>
      )}
      {review?.kind === "apply" && (
        <ReviewDialog
          title="应用模型"
          subtitle={`${review.profile.name} → ${review.agent.name}`}
          confirmLabel="应用模型"
          tone="primary"
          onClose={() => setReview(null)}
          onConfirm={() => apply(review.agent, review.profile)}
        >
          <p>目标：<code>{review.agent.config_path}</code></p>
          <p>只更新该 Agent 的 Model 所有字段，写入前创建备份。</p>
        </ReviewDialog>
      )}
    </>
  );
}

function ModelCard({
  profile,
  selected,
  agentIds,
  onOpen,
}: {
  profile: ModelProfileView;
  selected: boolean;
  agentIds: string[];
  onOpen: () => void;
}) {
  return (
    <ResourceCard
      className="mux-model-card"
      selected={selected}
      ariaLabel={`打开模型 ${profile.name} 详情`}
      onOpen={onOpen}
      identity={
        <>
        <Avatar seed={profile.name} label="M" size={36} />
        <div className="mux-model-card-identity">
          <div className="mux-model-card-name">
            <strong title={profile.name}>{profile.name}</strong>
            {profile.credential_saved && (
              <span className="mux-credential-mark" title="密钥已存入 Keychain">
                <CheckIcon className="w-3 h-3" />
              </span>
            )}
          </div>
          <code title={profile.model}>{profile.model}</code>
        </div>
        </>
      }
      configuration={
        <div className="mux-model-card-endpoint" title={profile.base_url}>
        <LinkIcon className="w-3 h-3 flex-shrink-0" />
        <span className="mux-model-card-endpoint-label">Base URL</span>
        <code>{profile.base_url}</code>
        </div>
      }
      state={
        <>
          <Badge tone="neutral">{protocolLabel(profile.protocol)}</Badge>
          {profile.reasoning && <Badge tone="info">Reasoning</Badge>}
          <Badge tone={profile.credential_saved ? "success" : "neutral"}>
            {profile.credential_saved ? "凭据已保存" : "无已存凭据"}
          </Badge>
        </>
      }
      impact={
        <AgentStack ids={agentIds} />
      }
    />
  );
}

function ModelInspector({
  profile,
  profiles,
  agents,
  busyAgent,
  onApply,
  onClose,
  onEdit,
  onDelete,
}: {
  profile: ModelProfileView;
  profiles: ModelProfileView[];
  agents: ModelAgentView[];
  busyAgent: string | null;
  onApply: (agent: ModelAgentView, profile: ModelProfileView) => void;
  onClose: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const assignedIds = agents
    .filter((agent) => agent.assigned_profile === profile.id)
    .map((agent) => agent.id);

  return (
    <ResourceInspector
      title={profile.name}
      avatar={<Avatar seed={profile.name} label="M" size={40} />}
      subtitle={<Badge tone="neutral">{protocolLabel(profile.protocol)}</Badge>}
      onClose={onClose}
      footer={
        <>
          <button className="btn-danger" type="button" onClick={onDelete}>
            <TrashIcon className="w-4 h-4" />
            删除
          </button>
          <div className="flex-1" />
          <button className="btn-primary" type="button" onClick={onEdit}>
            <EditIcon className="w-4 h-4" />
            编辑
          </button>
        </>
      }
    >
      <InspectorSection title="接口">
        <InspectorField label="模型 ID" mono>{profile.model}</InspectorField>
        <InspectorField label="Base URL" mono>{profile.base_url}</InspectorField>
        <InspectorField label="API Key">
          <span className={profile.credential_saved ? "mux-status-ok" : "mux-status-muted"}>
            {profile.credential_saved ? "已保存到 Keychain" : "未保存"}
          </span>
        </InspectorField>
      </InspectorSection>

      <InspectorSection title="Agent">
        <AgentStack ids={assignedIds} />
        <div className="mux-model-connection-list">
          {agents.map((agent) => {
            const compatible = agent.supported_protocols.includes(profile.protocol);
            const assigned = agent.assigned_profile === profile.id;
            const current = agent.assigned_profile
              ? profiles.find((item) => item.id === agent.assigned_profile)?.name
              : null;
            return (
              <div className="mux-model-connection-row" key={agent.id}>
                <AgentGlyph id={agent.id} name={agent.name} size={30} />
                <div className="min-w-0 flex-1">
                  <strong>{agent.name}</strong>
                  <span>
                    {assigned ? "当前使用" : current ? `当前：${current}` : agent.installed ? "尚未配置" : "未检测到"}
                  </span>
                </div>
                {agent.mode === "guided" ? (
                  <button type="button" className="btn-ghost" onClick={() => openUrl(agent.docs)}>
                    <LinkIcon className="w-4 h-4" />
                    设置
                  </button>
                ) : assigned ? (
                  <span className="mux-applied-label"><CheckIcon className="w-3.5 h-3.5" />已应用</span>
                ) : (
                  <button
                    type="button"
                    className="btn-secondary"
                    disabled={!compatible || busyAgent !== null}
                    title={compatible ? `应用到 ${agent.name}` : "协议不兼容"}
                    onClick={() => void onApply(agent, profile)}
                  >
                    {busyAgent === agent.id ? "应用中…" : compatible ? "应用" : "不兼容"}
                  </button>
                )}
              </div>
            );
          })}
        </div>
      </InspectorSection>
    </ResourceInspector>
  );
}

function ModelProfileDialog({
  initial,
  onClose,
  onSaved,
}: {
  initial: ModelProfileView | null;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const [draft, setDraft] = useState<ModelProfile>(initial ?? emptyProfile());
  const [credential, setCredential] = useState("");
  const [clearCredential, setClearCredential] = useState(false);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const valid =
    draft.id.trim() && draft.name.trim() && draft.base_url.trim() && draft.model.trim() && !busy;

  const save = async () => {
    if (!valid) return;
    setBusy(true);
    try {
      const credentialUpdate = clearCredential ? "" : credential || undefined;
      await saveModelProfile(
        {
          ...draft,
          id: draft.id.trim(),
          name: draft.name.trim(),
          base_url: draft.base_url.trim().replace(/\/$/, ""),
          model: draft.model.trim(),
        },
        credentialUpdate
      );
      await onSaved();
      toast.show({ kind: "success", msg: initial ? "模型已更新" : "模型已创建" });
    } catch (error) {
      toast.show({ kind: "error", msg: "保存失败：" + formatError(error) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <DialogShell
        kind="editor"
        size="md"
        title={initial ? "编辑模型" : "新建模型"}
        subtitle="API Key 保存在 macOS Keychain。"
        busy={busy}
        onClose={onClose}
        footerEnd={
          <>
            <button type="button" className="btn-ghost" disabled={busy} onClick={onClose}>取消</button>
            <button type="button" className="btn-primary" disabled={!valid} onClick={() => void save()}>
              {busy ? "保存中…" : "保存"}
            </button>
          </>
        }
      >
      <div className="mux-model-form">
        <div className="mux-model-form-grid">
          <label>
            <span>名称</span>
            <input
              autoFocus
              className="mux-model-field"
              value={draft.name}
              onChange={(event) => setDraft({ ...draft, name: event.target.value })}
              placeholder="公司网关"
            />
          </label>
          <label>
            <span>ID</span>
            <input
              className="mux-model-field"
              value={draft.id}
              disabled={Boolean(initial)}
              onChange={(event) => setDraft({ ...draft, id: event.target.value.toLowerCase() })}
              placeholder="company-gateway"
              spellCheck={false}
            />
          </label>
        </div>

        <label>
          <span>协议</span>
          <select
            className="mux-model-field"
            value={draft.protocol}
            onChange={(event) => setDraft({ ...draft, protocol: event.target.value as ModelProtocol })}
          >
            {PROTOCOLS.map((protocol) => (
              <option key={protocol.id} value={protocol.id}>{protocol.label}</option>
            ))}
          </select>
        </label>

        <label>
          <span>Base URL</span>
          <input
            className="mux-model-field"
            value={draft.base_url}
            onChange={(event) => setDraft({ ...draft, base_url: event.target.value })}
            placeholder="https://api.example.com/v1"
            spellCheck={false}
          />
        </label>

        <label>
          <span>模型 ID</span>
          <input
            className="mux-model-field"
            value={draft.model}
            onChange={(event) => setDraft({ ...draft, model: event.target.value })}
            placeholder="model-name"
            spellCheck={false}
          />
        </label>

        <label>
          <span>API Key</span>
          <input
            type="password"
            autoComplete="new-password"
            className="mux-model-field"
            value={credential}
            disabled={clearCredential}
            onChange={(event) => setCredential(event.target.value)}
            placeholder={initial?.credential_saved ? "留空保留现有密钥" : "本地无鉴权接口可留空"}
          />
        </label>

        {initial?.credential_saved && (
          <label className="mux-model-check">
            <input
              type="checkbox"
              checked={clearCredential}
              onChange={(event) => setClearCredential(event.target.checked)}
            />
            清除已存密钥
          </label>
        )}

        <details className="mux-model-advanced">
          <summary>Pi 高级设置</summary>
          <div className="mux-model-form-grid">
            <label>
              <span>上下文窗口</span>
              <input
                type="number"
                min={1}
                className="mux-model-field"
                value={draft.context_window ?? ""}
                onChange={(event) => setDraft({ ...draft, context_window: event.target.value ? Number(event.target.value) : undefined })}
                placeholder="128000"
              />
            </label>
            <label>
              <span>最大输出</span>
              <input
                type="number"
                min={1}
                className="mux-model-field"
                value={draft.max_output_tokens ?? ""}
                onChange={(event) => setDraft({ ...draft, max_output_tokens: event.target.value ? Number(event.target.value) : undefined })}
                placeholder="16384"
              />
            </label>
          </div>
          <label className="mux-model-check">
            <input
              type="checkbox"
              checked={draft.reasoning}
              onChange={(event) => setDraft({ ...draft, reasoning: event.target.checked })}
            />
            推理模型
          </label>
        </details>
      </div>
    </DialogShell>
  );
}
