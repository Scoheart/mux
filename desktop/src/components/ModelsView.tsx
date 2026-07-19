import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listModelProfiles } from "../lib/api";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import type {
  ModelProfile,
  ModelProfileView,
  ModelProtocol,
  ResourceNavigationIntent,
} from "../lib/types";
import { formatError } from "../lib/format";
import { Avatar, Badge } from "./ui";
import { ResourceCard } from "./ResourceCard";
import { ResourceState } from "./ResourceState";
import { DialogShell } from "./DialogShell";
import { AssetOperationReviewDialog } from "./AssetOperationReviewDialog";
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

type ModelAssetFilter = "all" | "credential" | "reasoning";

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
  consumptionState,
  intent,
  onIntentConsumed,
}: {
  consumptionState?: ConsumptionState;
  intent?: Extract<ResourceNavigationIntent, { domain: "model" }>;
  onIntentConsumed?(id: number): void;
} = {}) {
  const [profiles, setProfiles] = useState<ModelProfileView[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [editing, setEditing] = useState<ModelProfileView | null | undefined>(undefined);
  const [loading, setLoading] = useState(true);
  const [readError, setReadError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [assetFilter, setAssetFilter] = useState<ModelAssetFilter>("all");
  const [protocolFilter, setProtocolFilter] = useState<ModelProtocol | null>(null);
  const toast = useToast();
  const lastConsumedIntentId = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    const nextProfiles = await listModelProfiles();
    setProfiles(nextProfiles);
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
        toast.show({ kind: "error", msg: "读取模型资产失败：" + message });
      })
      .finally(() => setLoading(false));
  }, [refresh, toast]);

  const protocolCounts = useMemo(
    () => Object.fromEntries(
      PROTOCOLS.map((protocol) => [
        protocol.id,
        profiles.filter((profile) => profile.protocol === protocol.id).length,
      ])
    ) as Record<ModelProtocol, number>,
    [profiles],
  );

  const assetCounts = useMemo(() => {
    const scoped = protocolFilter
      ? profiles.filter((profile) => profile.protocol === protocolFilter)
      : profiles;
    return {
      all: scoped.length,
      credential: scoped.filter((profile) => profile.credential_saved).length,
      reasoning: scoped.filter((profile) => profile.reasoning).length,
    };
  }, [profiles, protocolFilter]);

  const filteredProfiles = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    return profiles.filter((profile) => {
      if (protocolFilter && profile.protocol !== protocolFilter) return false;
      if (assetFilter === "credential" && !profile.credential_saved) return false;
      if (assetFilter === "reasoning" && !profile.reasoning) return false;
      if (!needle) return true;
      return [profile.name, profile.id, profile.model, profile.base_url, protocolLabel(profile.protocol)]
        .join(" ")
        .toLocaleLowerCase()
        .includes(needle);
    });
  }, [assetFilter, profiles, protocolFilter, query]);

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
    setAssetFilter("all");
    setSelectedProfileId(profile?.id ?? null);
    if (!profile) toast.show({ kind: "error", msg: `未找到模型“${intent.profileId}”。` });
    onIntentConsumed?.(intent.id);
  }, [intent, loading, onIntentConsumed, profiles, toast]);

  const clearSelection = () => setSelectedProfileId(null);
  const planProfileDelete = async (profile: ModelProfileView) => {
    if (!consumptionState) return;
    try {
      await consumptionState.planDelete({ domain: "model", profile_id: profile.id });
    } catch (error) {
      toast.show({ kind: "error", msg: "无法生成删除计划：" + formatError(error) });
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
                onClick={() => { clearSelection(); setProtocolFilter(null); }}
              />
              {PROTOCOLS.map((protocol) => (
                <SidebarItem
                  key={protocol.id}
                  active={protocolFilter === protocol.id}
                  icon={<LayersIcon className="w-3.5 h-3.5" />}
                  label={protocol.label}
                  count={protocolCounts[protocol.id]}
                  onClick={() => { clearSelection(); setProtocolFilter(protocol.id); }}
                />
              ))}
            </SidebarSection>
          </WorkspaceSidebar>
        }
        query={query}
        onQueryChange={(value) => { clearSelection(); setQuery(value); }}
        searchPlaceholder="搜索模型资产"
        filters={
          <ResourceTabs
            label="模型资产"
            value={assetFilter}
            options={[
              { value: "all", label: "全部", count: assetCounts.all },
              { value: "credential", label: "有凭据", count: assetCounts.credential },
              { value: "reasoning", label: "推理模型", count: assetCounts.reasoning },
            ]}
            onChange={(value) => { clearSelection(); setAssetFilter(value); }}
          />
        }
        toolbarActions={
          <button className="btn-primary" type="button" disabled={!consumptionState} onClick={() => setEditing(null)}>
            <PlusIcon className="w-4 h-4" />
            新建模型
          </button>
        }
        inspector={selectedProfile ? (
          <ModelInspector
            profile={selectedProfile}
            onClose={clearSelection}
            onEdit={consumptionState ? () => setEditing(selectedProfile) : undefined}
            onDelete={consumptionState ? () => void planProfileDelete(selectedProfile) : undefined}
          />
        ) : undefined}
        onInspectorClose={clearSelection}
      >
        {loading ? (
          <ResourceState kind="loading" title="正在读取模型资产" />
        ) : readError ? (
          <ResourceState
            kind="read-error"
            icon={<LayersIcon className="w-6 h-6" />}
            title="读取模型资产失败"
            detail={readError}
            action={<button className="btn-primary" type="button" onClick={() => {
              setLoading(true);
              setReadError(null);
              void refresh()
                .catch((error) => setReadError(formatError(error)))
                .finally(() => setLoading(false));
            }}>重试</button>}
          />
        ) : filteredProfiles.length === 0 ? (
          <ResourceState
            kind={profiles.length === 0 ? "empty" : "no-match"}
            icon={<LayersIcon className="w-6 h-6" />}
            title={profiles.length === 0 ? "暂无模型资产" : "没有匹配项"}
            detail={profiles.length === 0 ? "新建一个可复用的模型配置。" : "调整搜索或筛选。"}
            action={profiles.length === 0 ? undefined : (
              <button className="btn-secondary" type="button" onClick={() => {
                setQuery("");
                setProtocolFilter(null);
                setAssetFilter("all");
              }}>清除筛选</button>
            )}
          />
        ) : (
          <ResourceGrid>
            {filteredProfiles.map((profile) => (
              <ModelCard
                key={profile.id}
                profile={profile}
                selected={profile.id === selectedProfileId}
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
          onReview={async (profile, credential) => {
            if (!consumptionState) throw new Error("中央资产事务不可用");
            await consumptionState.planUpdate({
              domain: "model",
              existing_id: editing?.id,
              profile,
              credential,
            });
            setEditing(undefined);
          }}
        />
      )}

      {consumptionState?.plan && (
        <AssetOperationReviewDialog
          plan={consumptionState.plan}
          busy={consumptionState.committing}
          error={consumptionState.error?.message}
          onCancel={consumptionState.cancel}
          onCommit={async (conflictConfirmation) => {
            const kind = consumptionState.plan?.kind;
            await consumptionState.commit(conflictConfirmation);
            await refresh();
            if (kind === "delete-asset") setSelectedProfileId(null);
            toast.show({
              kind: "success",
              msg: kind === "delete-asset" ? "模型资产已删除。" : "模型资产已保存。",
            });
          }}
        />
      )}
    </>
  );
}

function ModelCard({
  profile,
  selected,
  onOpen,
}: {
  profile: ModelProfileView;
  selected: boolean;
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
    />
  );
}

function ModelInspector({
  profile,
  onClose,
  onEdit,
  onDelete,
}: {
  profile: ModelProfileView;
  onClose: () => void;
  onEdit?: () => void;
  onDelete?: () => void;
}) {
  return (
    <ResourceInspector
      title={profile.name}
      avatar={<Avatar seed={profile.name} label="M" size={40} />}
      subtitle={<Badge tone="neutral">{protocolLabel(profile.protocol)}</Badge>}
      onClose={onClose}
      footer={
        <>
          <button className="btn-danger" type="button" disabled={!onDelete} onClick={onDelete}>
            <TrashIcon className="w-4 h-4" />
            删除
          </button>
          <div className="flex-1" />
          <button className="btn-primary" type="button" disabled={!onEdit} onClick={onEdit}>
            <EditIcon className="w-4 h-4" />
            编辑
          </button>
        </>
      }
    >
      <InspectorSection title="资产信息">
        <InspectorField label="资产 ID" mono>{profile.id}</InspectorField>
        <InspectorField label="协议">{protocolLabel(profile.protocol)}</InspectorField>
        <InspectorField label="推理">{profile.reasoning ? "支持" : "未标记"}</InspectorField>
      </InspectorSection>
      <InspectorSection title="接口">
        <InspectorField label="模型 ID" mono>{profile.model}</InspectorField>
        <InspectorField label="Base URL" mono>{profile.base_url}</InspectorField>
        <InspectorField label="API Key">
          <span className={profile.credential_saved ? "mux-status-ok" : "mux-status-muted"}>
            {profile.credential_saved ? "已保存到 Keychain" : "未保存"}
          </span>
        </InspectorField>
      </InspectorSection>
    </ResourceInspector>
  );
}

function ModelProfileDialog({
  initial,
  onClose,
  onReview,
}: {
  initial: ModelProfileView | null;
  onClose: () => void;
  onReview: (profile: ModelProfile, credential?: string) => Promise<void>;
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
      await onReview({
        ...draft,
        id: draft.id.trim(),
        name: draft.name.trim(),
        base_url: draft.base_url.trim().replace(/\/$/, ""),
        model: draft.model.trim(),
      }, credentialUpdate);
      setCredential("");
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
