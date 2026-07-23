import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listModelProfiles, listModelProviders } from "../lib/api";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import type {
  ModelProfile,
  ModelProfileView,
  ModelProviderView,
  ModelProtocol,
  ResourceNavigationIntent,
} from "../lib/types";
import { formatError } from "../lib/format";
import { Avatar, Badge } from "./ui";
import { ResourceCard, ResourceKindIcon } from "./ResourceCard";
import { ResourceState } from "./ResourceState";
import { DialogShell } from "./DialogShell";
import { AssetOperationReviewDialog } from "./AssetOperationReviewDialog";
import { FormSelect } from "./FormSelect";
import {
  CopyIcon,
  EditIcon,
  LayersIcon,
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
  { id: "openai-completions", label: "OpenAI Chat Completions" },
];

const CUSTOM_PROVIDER_OPTION = "__custom__";

type ModelAssetFilter = "all" | "credential" | "reasoning";

const emptyProfile = (): ModelProfile => ({
  id: "",
  name: "",
  provider: "",
  protocol: "openai-responses",
  base_url: "",
  model: "",
  reasoning: false,
});

function protocolLabel(protocol: ModelProtocol) {
  return PROTOCOLS.find((item) => item.id === protocol)?.label ?? protocol;
}

function providerLabel(providers: ModelProviderView[], provider: string) {
  return providers.find((item) => item.id === provider)?.name ?? (provider || "自动识别");
}

export function ModelsView({
  consumptionState,
  intent,
  onIntentConsumed,
  migrationCount = 0,
  onOpenMigration,
}: {
  consumptionState?: ConsumptionState;
  intent?: Extract<ResourceNavigationIntent, { domain: "model" }>;
  onIntentConsumed?(id: number): void;
  migrationCount?: number;
  onOpenMigration?(): void;
} = {}) {
  const [profiles, setProfiles] = useState<ModelProfileView[]>([]);
  const [providers, setProviders] = useState<ModelProviderView[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [editing, setEditing] = useState<ModelProfileView | null | undefined>(undefined);
  const [loading, setLoading] = useState(true);
  const [readError, setReadError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [assetFilter, setAssetFilter] = useState<ModelAssetFilter>("all");
  const [providerFilter, setProviderFilter] = useState<string | null>(null);
  const [protocolFilter, setProtocolFilter] = useState<ModelProtocol | null>(null);
  const toast = useToast();
  const lastConsumedIntentId = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    const [nextProfiles, nextProviders] = await Promise.all([
      listModelProfiles(),
      listModelProviders(),
    ]);
    setProfiles(nextProfiles);
    setProviders(nextProviders);
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

  const providerOptions = useMemo(() => {
    const known = new Map(providers.map((provider) => [provider.id, provider]));
    for (const profile of profiles) {
      if (!known.has(profile.provider)) {
        known.set(profile.provider, {
          id: profile.provider,
          name: profile.provider,
          default_base_url: null,
        });
      }
    }
    return [...known.values()].filter((provider) =>
      profiles.some((profile) => profile.provider === provider.id)
    );
  }, [profiles, providers]);

  const providerCounts = useMemo(
    () => Object.fromEntries(providerOptions.map((provider) => [
      provider.id,
      profiles.filter((profile) => profile.provider === provider.id).length,
    ])) as Record<string, number>,
    [profiles, providerOptions],
  );

  const assetCounts = useMemo(() => {
    const scoped = profiles.filter((profile) =>
      (!providerFilter || profile.provider === providerFilter) &&
      (!protocolFilter || profile.protocol === protocolFilter)
    );
    return {
      all: scoped.length,
      credential: scoped.filter((profile) => profile.credential_saved).length,
      reasoning: scoped.filter((profile) => profile.reasoning).length,
    };
  }, [profiles, protocolFilter, providerFilter]);

  const filteredProfiles = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    return profiles.filter((profile) => {
      if (protocolFilter && profile.protocol !== protocolFilter) return false;
      if (providerFilter && profile.provider !== providerFilter) return false;
      if (assetFilter === "credential" && !profile.credential_saved) return false;
      if (assetFilter === "reasoning" && !profile.reasoning) return false;
      if (!needle) return true;
      return [
        profile.name,
        profile.id,
        profile.model,
        profile.base_url,
        profile.provider,
        profile.model_vendor,
        profile.catalog_key,
        protocolLabel(profile.protocol),
      ]
        .join(" ")
        .toLocaleLowerCase()
        .includes(needle);
    });
  }, [assetFilter, profiles, protocolFilter, providerFilter, query]);

  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId) ?? null;

  useEffect(() => {
    if (!intent || loading || lastConsumedIntentId.current === intent.id) return;
    lastConsumedIntentId.current = intent.id;
    if (intent.kind === "create") {
      setSelectedProfileId(null);
      setEditing(null);
      onIntentConsumed?.(intent.id);
      return;
    }
    const profile = profiles.find((candidate) => candidate.id === intent.profileId);
    setQuery("");
    setProviderFilter(null);
    setProtocolFilter(null);
    setAssetFilter("all");
    setSelectedProfileId(profile?.id ?? null);
    if (!profile) toast.show({ kind: "error", msg: `未找到模型“${intent.profileId}”。` });
    onIntentConsumed?.(intent.id);
  }, [intent, loading, onIntentConsumed, profiles, toast]);

  const clearSelection = useCallback(() => {
    setSelectedProfileId(null);
    setEditing(undefined);
  }, []);
  const planProfileDelete = async (profile: ModelProfileView) => {
    if (!consumptionState) return;
    try {
      await consumptionState.planDelete({ domain: "model", profile_id: profile.id });
    } catch (error) {
      toast.show({ kind: "error", msg: "无法删除：" + formatError(error) });
    }
  };

  return (
    <>
      <ResourceWorkspace
        title="Models"
        description="集中管理模型连接、协议与凭据引用"
        sidebar={
          <WorkspaceSidebar title="Models" count={profiles.length}>
            <SidebarSection title="Provider">
              <SidebarItem
                active={providerFilter === null}
                icon={<LayersIcon className="w-3.5 h-3.5" />}
                label="全部 Provider"
                count={profiles.length}
                onClick={() => { clearSelection(); setProviderFilter(null); }}
              />
              {providerOptions.map((provider) => (
                <SidebarItem
                  key={provider.id}
                  active={providerFilter === provider.id}
                  icon={<LayersIcon className="w-3.5 h-3.5" />}
                  label={provider.name}
                  count={providerCounts[provider.id]}
                  onClick={() => { clearSelection(); setProviderFilter(provider.id); }}
                />
              ))}
            </SidebarSection>
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
          <>
            {migrationCount > 0 && onOpenMigration && (
              <button className="btn-secondary" type="button" onClick={onOpenMigration}>
                历史配置 {migrationCount}
              </button>
            )}
            <button className="btn-primary" type="button" disabled={!consumptionState} onClick={() => {
              clearSelection();
              setEditing(null);
            }}>
              <PlusIcon className="w-4 h-4" />
              新建模型
            </button>
          </>
        }
        inspector={selectedProfile && editing?.id === selectedProfile.id ? (
          <ModelProfileDialog
            initial={editing}
            providers={providers}
            presentation="inspector"
            onClose={clearSelection}
            onReview={async (profile, credential) => {
              if (!consumptionState) throw new Error("配置保存暂不可用");
              await consumptionState.planUpdate({
                domain: "model",
                existing_id: editing.id,
                profile,
                credential,
              });
              clearSelection();
            }}
          />
        ) : selectedProfile ? (
          <ModelInspector
            profile={selectedProfile}
            providerName={providerLabel(providers, selectedProfile.provider)}
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
                setProviderFilter(null);
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
                providerName={providerLabel(providers, profile.provider)}
                selected={profile.id === selectedProfileId}
                onOpen={() => {
                  setEditing(undefined);
                  setSelectedProfileId(profile.id);
                }}
              />
            ))}
          </ResourceGrid>
        )}
      </ResourceWorkspace>

      {editing === null && (
        <ModelProfileDialog
          initial={editing}
          providers={providers}
          onClose={() => setEditing(undefined)}
          onReview={async (profile, credential) => {
            if (!consumptionState) throw new Error("配置保存暂不可用");
            await consumptionState.planUpdate({
              domain: "model",
              existing_id: undefined,
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
          error={consumptionState.error}
          assetDisplayNames={Object.fromEntries(
            profiles.map((profile) => [`model:${profile.id}`, profile.name]),
          )}
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
  providerName,
  selected,
  onOpen,
}: {
  profile: ModelProfileView;
  providerName: string;
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
          <ResourceKindIcon kind="model" seed={profile.name} />
          <div className="mux-resource-card-copy">
            <div className="mux-resource-card-heading">
              <strong title={providerName}>{providerName}</strong>
            </div>
            <code className="mux-resource-card-code" title={profile.model}>{profile.model}</code>
          </div>
        </>
      }
      configuration={
        <span className="mux-resource-card-fact" title={protocolLabel(profile.protocol)}>
          {protocolLabel(profile.protocol)}
        </span>
      }
    />
  );
}

function ModelInspector({
  profile,
  providerName,
  onClose,
  onEdit,
  onDelete,
}: {
  profile: ModelProfileView;
  providerName: string;
  onClose: () => void;
  onEdit?: () => void;
  onDelete?: () => void;
}) {
  const toast = useToast();
  const copyProfileId = () => {
    navigator.clipboard.writeText(profile.id)
      .then(() => toast.show({ kind: "success", msg: "Profile ID 已复制。" }))
      .catch(() => toast.show({ kind: "error", msg: "复制失败。" }));
  };
  return (
    <ResourceInspector
      title={profile.name}
      avatar={<Avatar seed={profile.name} kind="model" size={40} />}
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
        <InspectorField label="Provider">{providerName}</InspectorField>
        {profile.model_vendor && <InspectorField label="模型开发商">{profile.model_vendor}</InspectorField>}
        <InspectorField label="协议">{protocolLabel(profile.protocol)}</InspectorField>
        <InspectorField label="推理">{profile.reasoning ? "支持" : "未标记"}</InspectorField>
      </InspectorSection>
      <InspectorSection title="接口">
        <InspectorField label="模型 ID" mono>{profile.model}</InspectorField>
        <InspectorField label="Base URL" mono>{profile.base_url}</InspectorField>
        {profile.env_key && <InspectorField label="环境变量" mono>{profile.env_key}</InspectorField>}
        <InspectorField label="API Key">
          <span className={profile.credential_saved ? "mux-status-ok" : "mux-status-muted"}>
            {profile.credential_saved ? "已保存到 Keychain" : "未保存"}
          </span>
        </InspectorField>
      </InspectorSection>
      <InspectorSection title="技术详情">
        <InspectorField label="Profile ID" mono>
          <span>{profile.id}</span>
          <button type="button" className="mux-copy-inline" aria-label="复制 Profile ID" onClick={copyProfileId}>
            <CopyIcon className="w-3.5 h-3.5" />
          </button>
        </InspectorField>
        <InspectorField label="Catalog Key" mono>{profile.catalog_key}</InspectorField>
      </InspectorSection>
    </ResourceInspector>
  );
}

function ModelProfileDialog({
  initial,
  providers,
  onClose,
  onReview,
  presentation = "dialog",
}: {
  initial: ModelProfileView | null;
  providers: ModelProviderView[];
  onClose: () => void;
  onReview: (profile: ModelProfile, credential?: string) => Promise<void>;
  presentation?: "dialog" | "inspector";
}) {
  const [draft, setDraft] = useState<ModelProfile>(initial ?? emptyProfile());
  const [credential, setCredential] = useState("");
  const [clearCredential, setClearCredential] = useState(false);
  const [busy, setBusy] = useState(false);
  const baseUrlAutoManaged = useRef(initial === null);
  const toast = useToast();
  const initialProvider = initial?.provider.trim() ?? "";
  const initialProviderIsKnown = providers.some(
    (provider) => provider.id !== "custom" && provider.id === initialProvider,
  );
  const [providerSelection, setProviderSelection] = useState(
    initialProvider && !initialProviderIsKnown ? CUSTOM_PROVIDER_OPTION : initialProvider,
  );
  const [customProvider, setCustomProvider] = useState(
    initialProvider && !initialProviderIsKnown ? initialProvider : "",
  );

  const updateProvider = (provider: string) => {
    setDraft((current) => ({
      ...current,
      provider,
      base_url: baseUrlAutoManaged.current
        ? providers.find((candidate) => candidate.id === provider.trim())?.default_base_url ?? ""
        : current.base_url,
    }));
  };

  const selectProvider = (provider: string) => {
    setProviderSelection(provider);
    updateProvider(provider === CUSTOM_PROVIDER_OPTION ? customProvider : provider);
  };

  const updateCustomProvider = (provider: string) => {
    setCustomProvider(provider);
    updateProvider(provider);
  };

  const updateBaseUrl = (baseUrl: string) => {
    baseUrlAutoManaged.current = false;
    setDraft((current) => ({ ...current, base_url: baseUrl }));
  };

  const valid = Boolean(
    draft.base_url.trim()
      && draft.model.trim()
      && (providerSelection !== CUSTOM_PROVIDER_OPTION || draft.provider.trim())
      && !busy,
  );

  const save = async () => {
    if (!valid) return;
    setBusy(true);
    try {
      const credentialUpdate = clearCredential ? "" : credential || undefined;
      await onReview({
        ...draft,
        id: initial?.id ?? "",
        name: draft.name.trim(),
        provider: draft.provider.trim(),
        model_vendor: draft.model_vendor?.trim() || undefined,
        base_url: draft.base_url.trim().replace(/\/$/, ""),
        model: draft.model.trim(),
        env_key: draft.env_key?.trim() || undefined,
      }, credentialUpdate);
      setCredential("");
    } catch (error) {
      toast.show({ kind: "error", msg: "保存失败：" + formatError(error) });
    } finally {
      setBusy(false);
    }
  };

  const footer = (
    <>
      <button type="button" className="btn-ghost" disabled={busy} onClick={onClose}>取消</button>
      <button type="button" className="btn-primary" disabled={!valid} onClick={() => void save()}>
        {busy ? "保存中…" : "保存"}
      </button>
    </>
  );
  const form = (
    <div className="mux-model-form">
        <div className="mux-model-form-grid">
          <div className="mux-model-form-field">
            <span>Provider</span>
            <FormSelect
              autoFocus
              ariaLabel="Provider"
              value={providerSelection}
              options={[
                { value: "", label: "自动识别" },
                ...providers
                  .filter((provider) => provider.id !== "custom")
                  .map((provider) => ({ value: provider.id, label: provider.name })),
                { value: CUSTOM_PROVIDER_OPTION, label: "Custom Provider…" },
              ]}
              onChange={selectProvider}
            />
            {providerSelection === CUSTOM_PROVIDER_OPTION && (
              <input
                aria-label="自定义 Provider ID"
                className="mux-model-field mux-model-custom-provider"
                value={customProvider}
                onChange={(event) => updateCustomProvider(event.target.value)}
                placeholder="例如 my-gateway"
                spellCheck={false}
              />
            )}
          </div>
          <label>
            <span>名称（可选）</span>
            <input
              className="mux-model-field"
              value={draft.name}
              onChange={(event) => setDraft({ ...draft, name: event.target.value })}
              placeholder="留空则根据模型自动生成"
            />
          </label>
        </div>

        <div className="mux-model-form-field">
          <span>协议</span>
          <FormSelect
            ariaLabel="协议"
            value={draft.protocol}
            options={PROTOCOLS.map((protocol) => ({ value: protocol.id, label: protocol.label }))}
            onChange={(protocol) => setDraft({ ...draft, protocol: protocol as ModelProtocol })}
          />
        </div>

        <label>
          <span>Base URL</span>
          <input
            aria-label="Base URL"
            className="mux-model-field"
            value={draft.base_url}
            onChange={(event) => updateBaseUrl(event.target.value)}
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
          <summary>高级设置</summary>
          <label>
            <span>模型开发商</span>
            <input
              className="mux-model-field"
              value={draft.model_vendor ?? ""}
              onChange={(event) => setDraft({ ...draft, model_vendor: event.target.value || undefined })}
              placeholder="自动推导，例如 anthropic"
              spellCheck={false}
            />
          </label>
          <label>
            <span>API Key 环境变量</span>
            <input
              className="mux-model-field"
              value={draft.env_key ?? ""}
              onChange={(event) => setDraft({ ...draft, env_key: event.target.value || undefined })}
              placeholder="MY_API_KEY"
              spellCheck={false}
            />
            <small>Grok Build 使用；变量值由启动环境提供，不从 Keychain 导出。</small>
          </label>
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
  );

  if (presentation === "inspector" && initial) {
    return (
      <ResourceInspector
        title={initial.name}
        avatar={<Avatar seed={initial.name} kind="model" size={40} />}
        subtitle={<Badge tone="neutral">编辑 · {protocolLabel(draft.protocol)}</Badge>}
        onClose={onClose}
        footer={
          <>
            <div className="flex-1" />
            {footer}
          </>
        }
      >
        {form}
      </ResourceInspector>
    );
  }

  return (
    <DialogShell
      kind="editor"
      size="md"
      title={initial ? "编辑模型" : "新建模型"}
      subtitle="API Key 保存在 macOS Keychain。"
      busy={busy}
      onClose={onClose}
      footerEnd={footer}
    >
      {form}
    </DialogShell>
  );
}
