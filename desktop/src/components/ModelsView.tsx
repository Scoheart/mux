import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  discoverProviderModels,
  listModelProfiles,
  listModelProviderInstances,
  listModelProviders,
  revealModelProviderCredential,
} from "../lib/api";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import type {
  ApiKeySource,
  ModelProfile,
  ModelProfileView,
  ModelProviderConfig,
  ModelProviderInstanceView,
  ModelProviderView,
  ModelProtocol,
  ProviderModelSummary,
  ResourceNavigationIntent,
} from "../lib/types";
import { formatError } from "../lib/format";
import {
  getCachedModelsDevMetadata,
  loadModelsDevMetadata,
  type ModelsDevMetadata,
} from "../lib/modelsDev";
import { Avatar, Badge } from "./ui";
import { ResourceState } from "./ResourceState";
import { DialogShell } from "./DialogShell";
import { AssetOperationReviewDialog } from "./AssetOperationReviewDialog";
import { FormSelect } from "./FormSelect";
import { ProviderGlyph } from "./providerIcons";
import {
  CalendarIcon,
  ChevronDownIcon,
  CopyIcon,
  EditIcon,
  EyeIcon,
  EyeOffIcon,
  GaugeIcon,
  KeyIcon,
  LayersIcon,
  LinkIcon,
  NetworkIcon,
  PlusIcon,
  RefreshIcon,
  SearchIcon,
  SparklesIcon,
  TerminalIcon,
  TrashIcon,
} from "./icons";
import { useToast } from "./Toast";
import {
  InspectorField,
  InspectorMetric,
  InspectorMetrics,
  ResourceInspector,
  ResourceWorkspace,
  SidebarItem,
  SidebarSection,
  WorkspaceSidebar,
} from "./ResourceWorkspace";

const PROTOCOLS: Array<{ id: ModelProtocol; label: string }> = [
  { id: "anthropic-messages", label: "Anthropic Messages" },
  { id: "openai-responses", label: "OpenAI Responses" },
  { id: "openai-completions", label: "OpenAI Chat Completions" },
  { id: "gemini-generate-content", label: "Gemini GenerateContent" },
];

const DEFAULT_ENDPOINT_PATHS: Record<ModelProtocol, string> = {
  "anthropic-messages": "/v1/messages",
  "openai-responses": "/responses",
  "openai-completions": "/chat/completions",
  "gemini-generate-content": "/models/{model}:generateContent",
};

const emptyProfile = (): ModelProfile => ({
  id: "",
  name: "",
  provider: "",
  protocol: "openai-responses",
  base_url: "",
  model: "",
});

type ProviderModelDiscoveryState = {
  status: "loading" | "success" | "error";
  models: ProviderModelSummary[];
  error?: string;
};

const MAX_VISIBLE_DISCOVERY_MODELS = 100;

function protocolLabel(protocol: ModelProtocol) {
  return PROTOCOLS.find((item) => item.id === protocol)?.label ?? protocol;
}

function providerLabel(providers: ModelProviderView[], provider: string) {
  return providers.find((item) => item.id === provider)?.name ?? (provider || "Custom Provider");
}

function isCustomProviderInstance(
  instance: ModelProviderInstanceView,
  templates: ModelProviderView[],
) {
  return instance.provider === "custom"
    || templates.find((template) => template.id === instance.provider)?.category === "custom";
}

function partitionProviderInstances(
  instances: ModelProviderInstanceView[],
  templates: ModelProviderView[],
) {
  const official: ModelProviderInstanceView[] = [];
  const custom: ModelProviderInstanceView[] = [];
  for (const instance of instances) {
    (isCustomProviderInstance(instance, templates) ? custom : official).push(instance);
  }
  return { official, custom };
}

const SIDEBAR_PROVIDER_PREVIEW = 5;

function previewProviderInstances(
  items: ModelProviderInstanceView[],
  expanded: boolean,
  selectedId: string | null,
) {
  if (expanded || items.length <= SIDEBAR_PROVIDER_PREVIEW) return items;
  const preview = items.slice(0, SIDEBAR_PROVIDER_PREVIEW);
  const selected = selectedId ? items.find((item) => item.id === selectedId) : undefined;
  if (selected && !preview.some((item) => item.id === selected.id)) {
    return [...preview, selected];
  }
  return preview;
}

function ProviderSidebarGroup({
  title,
  items,
  selectedId,
  onSelect,
}: {
  title: string;
  items: ModelProviderInstanceView[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const visible = previewProviderInstances(items, expanded, selectedId);
  if (items.length === 0) return null;
  const hiddenCount = Math.max(0, items.length - SIDEBAR_PROVIDER_PREVIEW);

  return (
    <SidebarSection title={title}>
      {visible.map((provider) => (
        <SidebarItem
          key={provider.id}
          active={selectedId === provider.id}
          icon={<ProviderGlyph id={provider.provider} name={provider.name} size={18} />}
          label={provider.name}
          count={provider.model_count}
          onClick={() => onSelect(provider.id)}
        />
      ))}
      {hiddenCount > 0 && (
        <button
          type="button"
          className="mux-sidebar-more"
          aria-expanded={expanded}
          onClick={() => setExpanded((value) => !value)}
        >
          <ChevronDownIcon className="mux-sidebar-more-icon" />
          {expanded
            ? t("models.showLessProviders")
            : t("models.showMoreProviders", { count: hiddenCount })}
        </button>
      )}
    </SidebarSection>
  );
}

function providerTemplateConnection(provider: ModelProviderView | null | undefined) {
  return {
    base_url: provider?.base_url ?? "",
    protocols: provider
      ? Object.fromEntries(
          Object.entries(provider.protocols)
            .map(([protocol, config]) => [protocol, { ...config! }]),
        ) as ModelProviderConfig["protocols"]
      : {},
  };
}

function providerTemplatePath(
  provider: ModelProviderView | null | undefined,
  protocol: ModelProtocol,
) {
  return provider?.protocols[protocol]?.endpoint_path ?? DEFAULT_ENDPOINT_PATHS[protocol];
}

function normalizeBaseUrl(value: string) {
  const normalized = value.trim().replace(/\/+$/, "");
  if (!normalized || /\s/.test(normalized)) return null;
  try {
    const url = new URL(normalized);
    if (
      !["http:", "https:"].includes(url.protocol)
      || !url.hostname
      || url.username
      || url.password
      || url.search
      || url.hash
    ) return null;
    return normalized;
  } catch {
    return null;
  }
}

function normalizeModelCatalogUrl(value: string) {
  const normalized = value.trim();
  if (!normalized || /\s/.test(normalized)) return null;
  try {
    const url = new URL(normalized);
    if (
      !["http:", "https:"].includes(url.protocol)
      || !url.hostname
      || url.username
      || url.password
      || url.hash
    ) return null;
    return normalized;
  } catch {
    return null;
  }
}

function normalizeEndpointPath(value: string) {
  const trimmed = value.trim();
  if (
    !trimmed
    || /\s/.test(trimmed)
    || trimmed.includes("#")
    || trimmed.includes("?")
    || trimmed.includes("://")
    || trimmed.startsWith("//")
    || trimmed.includes("\\")
  ) return null;
  const normalized = `/${trimmed.replace(/^\/+/, "")}`;
  if (normalized.split("/").some((segment) => {
    const lower = segment.toLocaleLowerCase();
    const decodedDots = lower.replaceAll("%2e", ".");
    return decodedDots === "." || decodedDots === ".."
      || lower.includes("%2f") || lower.includes("%5c");
  })) {
    return null;
  }
  const first = normalized.replace(/^\/+/, "").split("/", 1)[0];
  if (!trimmed.startsWith("/") && (first.includes(".") || first.includes(":"))) return null;
  return normalized;
}

function fullRequestUrl(baseUrl: string, endpointPath: string) {
  const base = normalizeBaseUrl(baseUrl);
  const path = normalizeEndpointPath(endpointPath);
  return base && path ? `${base}${path}` : "";
}

function profileProviderName(
  profile: ModelProfileView,
  instances: ModelProviderInstanceView[],
  providers: ModelProviderView[],
) {
  return instances.find((item) => item.id === profile.provider_id)?.name
    ?? providerLabel(providers, profile.provider);
}

function formatTokens(value: number) {
  if (value >= 1_000_000) {
    return `${Number((value / 1_000_000).toFixed(value % 1_000_000 === 0 ? 0 : 2))}M`;
  }
  if (value >= 1_000) {
    return `${Number((value / 1_000).toFixed(value % 1_000 === 0 ? 0 : 1))}K`;
  }
  return String(value);
}

function formatCatalogCost(value: number) {
  return `$${Number(value.toFixed(value < 0.01 ? 4 : 2))}/M`;
}

function readableModelName(
  profile: ModelProfileView,
  providerName: string,
  metadata?: ModelsDevMetadata,
) {
  const profileName = profile.name.trim();
  const isProviderPlaceholder = [providerName, profile.provider]
    .some((candidate) => candidate.trim().toLocaleLowerCase() === profileName.toLocaleLowerCase());
  return ((!profileName || isProviderPlaceholder ? metadata?.name : profileName) || profileName || profile.model);
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
  const [providers, setProviders] = useState<ModelProviderView[]>([]);
  const [providerInstances, setProviderInstances] = useState<ModelProviderInstanceView[]>([]);
  const [providerFilter, setProviderFilter] = useState<string | null>(null);
  const [providerCatalogOpen, setProviderCatalogOpen] = useState(false);
  const [creatingForProviderId, setCreatingForProviderId] = useState<string | null>(null);
  const [creatingProviderTemplate, setCreatingProviderTemplate] = useState<ModelProviderView | null>(null);
  const [editingProvider, setEditingProvider] = useState<ModelProviderInstanceView | null | undefined>(undefined);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [editing, setEditing] = useState<ModelProfileView | null | undefined>(undefined);
  const [loading, setLoading] = useState(true);
  const [readError, setReadError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [modelsDevByProfileId, setModelsDevByProfileId] = useState<Record<string, ModelsDevMetadata>>({});
  const toast = useToast();
  const showToast = toast.show;
  const { t } = useTranslation();
  const lastConsumedIntentId = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    const [nextProfiles, nextProviders, nextProviderInstances] = await Promise.all([
      listModelProfiles(),
      listModelProviders(),
      listModelProviderInstances(),
    ]);
    setProfiles(nextProfiles);
    setProviders(nextProviders);
    setProviderInstances(nextProviderInstances);
    setProviderFilter((current) =>
      current && nextProviderInstances.some((provider) => provider.id === current) ? current : null
    );
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
        showToast({ kind: "error", msg: t("models.readFailed", { error: message }) });
      })
      .finally(() => setLoading(false));
  }, [refresh, showToast, t]);

  useEffect(() => {
    let active = true;
    setModelsDevByProfileId(getCachedModelsDevMetadata(profiles));
    if (profiles.length > 0) {
      void loadModelsDevMetadata(profiles).then((metadata) => {
        if (active) setModelsDevByProfileId(metadata);
      });
    }
    return () => { active = false; };
  }, [profiles]);

  const filteredProfiles = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    return profiles.filter((profile) => {
      if (providerFilter && profile.provider_id !== providerFilter) return false;
      if (!needle) return true;
      return [
        profile.name,
        profile.id,
        profile.model,
        profile.base_url,
        profile.provider,
        profileProviderName(profile, providerInstances, providers),
        profile.catalog_key,
        protocolLabel(profile.protocol),
      ]
        .join(" ")
        .toLocaleLowerCase()
        .includes(needle);
    });
  }, [profiles, providerFilter, providerInstances, providers, query]);

  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId) ?? null;
  const selectedProvider = providerInstances.find((provider) => provider.id === providerFilter) ?? null;
  const { official: officialProviders, custom: customProviders } = useMemo(
    () => partitionProviderInstances(providerInstances, providers),
    [providerInstances, providers],
  );

  useEffect(() => {
    if (!intent || loading || lastConsumedIntentId.current === intent.id) return;
    lastConsumedIntentId.current = intent.id;
    if (intent.kind === "create") {
      setSelectedProfileId(null);
      setProviderCatalogOpen(true);
      onIntentConsumed?.(intent.id);
      return;
    }
    const profile = profiles.find((candidate) => candidate.id === intent.profileId);
    setQuery("");
    setProviderFilter(null);
    setSelectedProfileId(profile?.id ?? null);
    if (!profile) toast.show({ kind: "error", msg: t("models.notFound", { id: intent.profileId }) });
    onIntentConsumed?.(intent.id);
  }, [intent, loading, onIntentConsumed, profiles, t, toast]);

  const clearSelection = useCallback(() => {
    setSelectedProfileId(null);
    setEditing(undefined);
    setCreatingForProviderId(null);
    setCreatingProviderTemplate(null);
  }, []);
  const selectProvider = (id: string | null) => {
    clearSelection();
    setProviderFilter(id);
  };
  const planProfileDelete = async (profile: ModelProfileView) => {
    if (!consumptionState) return;
    try {
      await consumptionState.planDelete({ domain: "model", profile_id: profile.id });
    } catch (error) {
      toast.show({ kind: "error", msg: t("models.cannotDelete", { error: formatError(error) }) });
    }
  };

  return (
    <div className="mux-models-workspace">
      <ResourceWorkspace
        title={t("models.title")}
        description={t("models.description")}
        sidebar={
          <WorkspaceSidebar title={t("models.title")} count={profiles.length}>
            <SidebarSection title={t("models.library")}>
              <SidebarItem
                active={providerFilter === null}
                icon={<LayersIcon className="w-3.5 h-3.5" />}
                label={t("models.allModels")}
                count={profiles.length}
                onClick={() => selectProvider(null)}
              />
            </SidebarSection>
            <ProviderSidebarGroup
              title={t("models.customProviders")}
              items={customProviders}
              selectedId={providerFilter}
              onSelect={selectProvider}
            />
            <ProviderSidebarGroup
              title={t("models.officialProviders")}
              items={officialProviders}
              selectedId={providerFilter}
              onSelect={selectProvider}
            />
          </WorkspaceSidebar>
        }
        query={query}
        onQueryChange={(value) => { clearSelection(); setQuery(value); }}
        searchPlaceholder={t("models.search")}
        toolbarActions={
          <>
            <button className="btn-secondary" type="button" disabled={!consumptionState} onClick={() => {
              clearSelection();
              setProviderCatalogOpen(true);
            }}>
              <PlusIcon className="w-4 h-4" />
              {t("models.addProvider")}
            </button>
            <button className="btn-primary" type="button" disabled={!consumptionState} onClick={() => {
              if (providerInstances.length === 0) {
                setProviderCatalogOpen(true);
                toast.show({ kind: "error", msg: t("models.providerRequired") });
                return;
              }
                clearSelection();
                setCreatingForProviderId(providerFilter ?? providerInstances[0].id);
                setEditing(null);
              }}>
                <PlusIcon className="w-4 h-4" />
                {t("models.addModel")}
              </button>
          </>
        }
        inspector={selectedProfile && editing?.id === selectedProfile.id ? (
          <ModelProfileDialog
            initial={editing}
            providerInstances={providerInstances}
            preferredProviderId={editing.provider_id}
            presentation="inspector"
            onClose={clearSelection}
            onReview={async (profile) => {
              if (!consumptionState) throw new Error(t("models.saveUnavailable"));
              await consumptionState.planUpdate({
                domain: "model",
                existing_id: editing.id,
                profile,
              });
              clearSelection();
            }}
          />
        ) : selectedProfile ? (
          <ModelInspector
            profile={selectedProfile}
            providerName={profileProviderName(selectedProfile, providerInstances, providers)}
            provider={providerInstances.find((provider) => provider.id === selectedProfile.provider_id) ?? null}
            metadata={modelsDevByProfileId[selectedProfile.id]}
            onClose={clearSelection}
            onEdit={consumptionState ? () => setEditing(selectedProfile) : undefined}
            onDelete={consumptionState ? () => void planProfileDelete(selectedProfile) : undefined}
          />
        ) : undefined}
        onInspectorClose={clearSelection}
      >
        {consumptionState?.plan ? (
          <AssetOperationReviewDialog
            plan={consumptionState.plan}
            busy={consumptionState.committing}
            error={consumptionState.error}
            assetDisplayNames={Object.fromEntries(
              [
                ...profiles.map((profile) => [`model:${profile.id}`, profile.name] as const),
                ...providerInstances.map((provider) => [
                  `model-provider:${provider.id}`,
                  provider.name,
                ] as const),
              ],
            )}
            onCancel={consumptionState.cancel}
            onCommit={async () => {
              const kind = consumptionState.plan?.kind;
              await consumptionState.commit();
              await refresh();
              if (kind === "delete-asset") setSelectedProfileId(null);
              toast.show({
                kind: "success",
                msg: kind === "delete-asset" ? t("models.deleted") : t("models.saved"),
              });
            }}
          />
        ) : <>
        {selectedProvider && (
          <ProviderBanner
            provider={selectedProvider}
            onEdit={consumptionState ? () => setEditingProvider(selectedProvider) : undefined}
            onDelete={consumptionState ? async () => {
              try {
                await consumptionState.planDelete({
                  domain: "model-provider",
                  provider_id: selectedProvider.id,
                });
              } catch (error) {
                toast.show({
                  kind: "error",
                  msg: t("models.cannotDeleteProvider", { error: formatError(error) }),
                });
              }
            } : undefined}
          />
        )}
        {loading ? (
          <ResourceState kind="loading" title={t("models.loading")} />
        ) : readError ? (
          <ResourceState
            kind="read-error"
            icon={<LayersIcon className="w-6 h-6" />}
            title={t("models.readFailedTitle")}
            detail={readError}
            action={<button className="btn-primary" type="button" onClick={() => {
              setLoading(true);
              setReadError(null);
              void refresh()
                .catch((error) => setReadError(formatError(error)))
                .finally(() => setLoading(false));
            }}>{t("common.retry")}</button>}
          />
        ) : filteredProfiles.length === 0 ? (
          <ResourceState
            kind={profiles.length === 0 ? "empty" : "no-match"}
            icon={<LayersIcon className="w-6 h-6" />}
            title={profiles.length === 0 ? t("models.empty") : t("models.noMatches")}
            detail={profiles.length === 0 ? t("models.emptyDetail") : t("models.noMatchesDetail")}
            action={profiles.length === 0 ? undefined : (
              <button className="btn-secondary" type="button" onClick={() => {
                setQuery("");
                setProviderFilter(null);
              }}>{t("models.clearFilters")}</button>
            )}
          />
        ) : (
          <ModelList
            profiles={filteredProfiles}
            providerInstances={providerInstances}
            providers={providers}
            metadata={modelsDevByProfileId}
            selectedProfileId={selectedProfileId}
            onOpen={(profileId) => {
              setEditing(undefined);
              setSelectedProfileId(profileId);
            }}
          />
        )}
        </>}
      </ResourceWorkspace>

      {providerCatalogOpen && (
        <ProviderCatalogDialog
          providers={providers}
          onClose={() => setProviderCatalogOpen(false)}
          onUse={(provider) => {
            setProviderCatalogOpen(false);
            setCreatingProviderTemplate(provider);
            setEditingProvider(null);
          }}
        />
      )}

      {editing === null && (
        <ModelProfileDialog
          initial={editing}
          providerInstances={providerInstances}
          preferredProviderId={creatingForProviderId}
          providerSelectionLocked={providerFilter !== null}
          onClose={() => {
            setEditing(undefined);
            setCreatingProviderTemplate(null);
          }}
          onAddProvider={() => {
            setEditing(undefined);
            setCreatingForProviderId(null);
            setProviderCatalogOpen(true);
          }}
          onReview={async (profile) => {
            if (!consumptionState) throw new Error(t("models.saveUnavailable"));
            await consumptionState.planUpdate({
              domain: "model",
              existing_id: undefined,
              profile,
            });
            setEditing(undefined);
            setCreatingForProviderId(null);
            setCreatingProviderTemplate(null);
          }}
        />
      )}

      {editingProvider !== undefined && (
        <ModelProviderDialog
          initial={editingProvider}
          providerTemplate={creatingProviderTemplate}
          providers={providers}
          onClose={() => {
            setEditingProvider(undefined);
            setCreatingProviderTemplate(null);
          }}
          onReview={async (provider, credential) => {
            if (!consumptionState) throw new Error(t("models.saveUnavailable"));
            await consumptionState.planUpdate({
              domain: "model-provider",
              existing_id: editingProvider?.id,
              provider,
              credential,
            });
            setEditingProvider(undefined);
            setCreatingProviderTemplate(null);
          }}
        />
      )}

    </div>
  );
}

function ProviderBanner({
  provider,
  onEdit,
  onDelete,
}: {
  provider: ModelProviderInstanceView;
  onEdit?: () => void;
  onDelete?: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="mux-model-provider-banner">
      <div className="mux-model-provider-banner-identity">
        <ProviderGlyph id={provider.provider} name={provider.name} size={30} />
        <div className="mux-model-provider-banner-copy">
          <strong>{provider.name}</strong>
          <span>{t("models.providerModelCount", { count: provider.model_count })}</span>
          <span
            className="mux-model-provider-credential"
            data-saved={provider.credential_saved ? "true" : "false"}
            role="img"
            aria-label={provider.credential_saved ? t("models.keychainSaved") : t("models.keychainNotSaved")}
            title={provider.credential_saved ? t("models.keychainSaved") : t("models.keychainNotSaved")}
          >
            <KeyIcon className="w-3.5 h-3.5" />
          </span>
        </div>
      </div>
      <div className="flex items-center gap-2">
        <button className="btn-danger" type="button" disabled={!onDelete} onClick={onDelete}>
          <TrashIcon className="w-4 h-4" />
          {t("common.delete")}
        </button>
        <button className="btn-secondary" type="button" disabled={!onEdit} onClick={onEdit}>
          <EditIcon className="w-4 h-4" />
          {t("models.editProvider")}
        </button>
      </div>
    </div>
  );
}

function ModelList({
  profiles,
  providerInstances,
  providers,
  metadata,
  selectedProfileId,
  onOpen,
}: {
  profiles: ModelProfileView[];
  providerInstances: ModelProviderInstanceView[];
  providers: ModelProviderView[];
  metadata: Record<string, ModelsDevMetadata>;
  selectedProfileId: string | null;
  onOpen: (profileId: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="mux-asset-list mux-model-list" role="list" aria-label={t("models.asset")}>
      {profiles.map((profile) => {
        const providerName = profileProviderName(profile, providerInstances, providers);
        const profileMetadata = metadata[profile.id];
        const displayName = readableModelName(profile, providerName, profileMetadata);
        const contextWindow = profile.context_window ?? profileMetadata?.contextWindow;
        return (
          <div role="listitem" key={profile.id}>
            <button
              type="button"
              className="mux-asset-list-row mux-model-list-row"
              data-selected={profile.id === selectedProfileId ? "true" : undefined}
              aria-label={t("models.openDetails", { name: profile.name })}
              onClick={() => onOpen(profile.id)}
            >
              <span className="mux-asset-list-identity mux-model-list-identity">
                <ProviderGlyph id={profile.provider || "custom"} name={providerName} size={34} />
                <span className="mux-asset-list-copy">
                  <strong title={displayName}>{displayName}</strong>
                  <span className="mux-model-list-subline">
                    <code title={profile.model}>{profile.model}</code>
                    {contextWindow && (
                      <span
                        className="mux-model-list-context"
                        aria-label={`${t("models.context")} ${formatTokens(contextWindow)}`}
                        title={`${t("models.contextWindow")}: ${contextWindow.toLocaleString()} tokens`}
                      >
                        <span>{t("models.context")}</span>
                        <strong>{formatTokens(contextWindow)}</strong>
                      </span>
                    )}
                  </span>
                </span>
              </span>
            </button>
          </div>
        );
      })}
    </div>
  );
}

function ModelInspector({
  profile,
  providerName,
  provider,
  metadata,
  onClose,
  onEdit,
  onDelete,
}: {
  profile: ModelProfileView;
  providerName: string;
  provider: ModelProviderInstanceView | null;
  metadata?: ModelsDevMetadata;
  onClose: () => void;
  onEdit?: () => void;
  onDelete?: () => void;
}) {
  const { t } = useTranslation();
  const toast = useToast();
  const contextWindow = profile.context_window ?? metadata?.contextWindow;
  const maxOutputTokens = profile.max_output_tokens ?? metadata?.maxOutputTokens;
  const capabilities = [
    metadata?.toolCall && t("models.tools"),
    metadata?.structuredOutput && t("models.structuredOutput"),
    metadata?.modalities?.some((modality) => modality !== "text") && t("models.multimodal"),
  ].filter((item): item is string => Boolean(item));
  const showReasoning = profile.reasoning !== undefined || metadata?.reasoning === true;
  const requestUrl = provider
    ? fullRequestUrl(
        provider.base_url,
        provider.protocols[profile.protocol]?.endpoint_path ?? "",
      )
    : "";
  const priceSummary = [
    metadata?.inputCost != null && t("models.inputMetric", { value: formatCatalogCost(metadata.inputCost) }),
    metadata?.outputCost != null && t("models.outputPriceMetric", { value: formatCatalogCost(metadata.outputCost) }),
  ].filter(Boolean).join(" · ");
  const copyValue = async (label: string, value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      toast.show({ kind: "success", msg: t("models.copiedValue", { label }) });
    } catch (error) {
      toast.show({ kind: "error", msg: t("models.copyValueFailed", { error: formatError(error) }) });
    }
  };
  const copyAction = (label: string, value: string) => (
    <button
      type="button"
      aria-label={t("models.copyValue", { label })}
      title={t("models.copyValue", { label })}
      onClick={() => void copyValue(label, value)}
    >
      <CopyIcon className="w-3.5 h-3.5" />
    </button>
  );
  return (
    <ResourceInspector
      title={readableModelName(profile, providerName, metadata)}
      avatar={<Avatar seed={profile.name} kind="model" size={40} />}
      subtitle={<Badge tone="neutral">{protocolLabel(profile.protocol)}</Badge>}
      onClose={onClose}
      footer={
        <>
          <button className="btn-danger" type="button" disabled={!onDelete} onClick={onDelete}>
            <TrashIcon className="w-4 h-4" />
            {t("common.delete")}
          </button>
          <div className="flex-1" />
          <button className="btn-primary" type="button" disabled={!onEdit} onClick={onEdit}>
            <EditIcon className="w-4 h-4" />
            {t("common.edit")}
          </button>
        </>
      }
    >
      {(contextWindow || maxOutputTokens || capabilities.length > 0 || priceSummary) && (
        <InspectorMetrics>
          {contextWindow && (
            <InspectorMetric icon={<LayersIcon />} label={t("models.context")} value={formatTokens(contextWindow)} />
          )}
          {maxOutputTokens && (
            <InspectorMetric icon={<GaugeIcon />} label={t("models.outputLimit")} value={formatTokens(maxOutputTokens)} />
          )}
          {capabilities.length > 0 && (
            <InspectorMetric icon={<SparklesIcon />} label={t("models.capabilities")} value={capabilities.join(" · ")} />
          )}
          {priceSummary && (
            <InspectorMetric icon={<KeyIcon />} label={t("models.catalogPrice")} value={priceSummary} />
          )}
        </InspectorMetrics>
      )}
      {metadata?.description && <p className="mux-model-inspector-description">{metadata.description}</p>}
      <section className="mux-model-inspector-fields" aria-label={t("models.detailsFields")}>
        <InspectorField icon={<LayersIcon />} label={t("models.provider")}>{providerName}</InspectorField>
        <InspectorField icon={<NetworkIcon />} label={t("models.protocol")}>{protocolLabel(profile.protocol)}</InspectorField>
        {showReasoning && (
          <InspectorField icon={<SparklesIcon />} label={t("models.reasoningMode")}>
            {profile.reasoning === undefined
              ? t("models.reasoningAuto")
              : profile.reasoning
                ? t("models.reasoningOn")
                : t("models.reasoningOff")}
          </InspectorField>
        )}
        {metadata?.releaseDate && <InspectorField icon={<CalendarIcon />} label={t("models.releaseDate")}>{metadata.releaseDate}</InspectorField>}
        <InspectorField icon={<TerminalIcon />} label={t("models.modelId")} mono wide action={copyAction(t("models.modelId"), profile.model)}>{profile.model}</InspectorField>
        <InspectorField
          icon={<LinkIcon />}
          label={t("models.fullRequestUrl")}
          mono
          wide
          action={requestUrl ? copyAction(t("models.fullRequestUrl"), requestUrl) : undefined}
        >
          {requestUrl || t("common.notSet")}
        </InspectorField>
        {profile.env_key && <InspectorField icon={<KeyIcon />} label={t("models.environmentVariable")} mono wide>{profile.env_key}</InspectorField>}
      </section>
    </ResourceInspector>
  );
}

function ProviderCatalogDialog({
  providers,
  onClose,
  onUse,
}: {
  providers: ModelProviderView[];
  onClose: () => void;
  onUse: (provider: ModelProviderView) => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState(
    providers.find((provider) => provider.id === "custom")?.id ?? providers[0]?.id ?? "",
  );
  const orderedProviders = [
    ...providers.filter((provider) => provider.id === "custom"),
    ...providers.filter((provider) => provider.id !== "custom"),
  ];
  const visibleProviders = orderedProviders.filter((provider) => {
    const needle = query.trim().toLocaleLowerCase();
    return !needle || [
      provider.name,
      provider.id,
      provider.base_url ?? "",
      ...Object.values(provider.protocols).flatMap((config) => [
        config?.endpoint_path ?? "",
        fullRequestUrl(provider.base_url ?? "", config?.endpoint_path ?? ""),
      ]),
      provider.default_base_url ?? "",
      ...provider.additional_endpoints.map(({ base_url }) => base_url),
    ]
      .join(" ")
      .toLocaleLowerCase()
      .includes(needle);
  });
  const selected = visibleProviders.find((provider) => provider.id === selectedId) ?? null;

  return (
    <DialogShell
      className="mux-dialog-provider-catalog"
      kind="picker"
      size="lg"
      title={t("models.providerCatalogTitle")}
      onClose={onClose}
      footerStart={selected ? (
        <span className="mux-provider-catalog-selection">
          <span aria-hidden="true">✓</span>
          <strong>{selected.name}</strong>
        </span>
      ) : undefined}
      footerEnd={(
        <>
          <button type="button" className="btn-ghost" onClick={onClose}>{t("common.cancel")}</button>
          <button
            type="button"
            className="btn-primary"
            disabled={!selected}
            onClick={() => selected && onUse(selected)}
          >
            {t("models.useProviderTemplate")}
          </button>
        </>
      )}
    >
      <div className="mux-provider-catalog">
        <label className="mux-provider-catalog-search">
          <SearchIcon className="w-4 h-4" />
          <input
            autoFocus
            type="search"
            className="mux-model-field"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("models.searchProviders")}
            aria-label={t("models.searchProviders")}
          />
        </label>
        <div className="mux-provider-catalog-grid" role="radiogroup" aria-label={t("models.providerCatalogTitle")}>
          {visibleProviders.map((provider) => (
            <button
              type="button"
              role="radio"
              aria-checked={provider.id === selectedId}
              data-selected={provider.id === selectedId ? "true" : undefined}
              className="mux-provider-catalog-item"
              key={provider.id}
              onClick={() => setSelectedId(provider.id)}
            >
              <span className="mux-provider-catalog-icon">
                <ProviderGlyph id={provider.id} name={provider.name} size={26} />
              </span>
              <span className="mux-provider-catalog-copy">
                <strong>{provider.name}</strong>
                <code>
                  {provider.base_url
                    ? fullRequestUrl(
                        provider.base_url,
                        provider.protocols[provider.default_protocol]?.endpoint_path
                          ?? DEFAULT_ENDPOINT_PATHS[provider.default_protocol],
                      )
                    : t("models.providerEndpointRequired")}
                </code>
              </span>
              <span className="mux-provider-catalog-check" aria-hidden="true">✓</span>
            </button>
          ))}
        </div>
        {visibleProviders.length === 0 && (
          <div className="mux-provider-catalog-empty">{t("models.noProviderMatches")}</div>
        )}
      </div>
    </DialogShell>
  );
}

function ModelProfileDialog({
  initial,
  providerInstances,
  preferredProviderId,
  providerSelectionLocked = false,
  onClose,
  onReview,
  onAddProvider,
  presentation = "dialog",
}: {
  initial: ModelProfileView | null;
  providerInstances: ModelProviderInstanceView[];
  preferredProviderId?: string | null;
  providerSelectionLocked?: boolean;
  onClose: () => void;
  onReview: (profile: ModelProfile) => Promise<void>;
  onAddProvider?: () => void;
  presentation?: "dialog" | "inspector";
}) {
  const { t } = useTranslation();
  const preferredProvider = providerInstances.find((provider) =>
    provider.id === (initial?.provider_id ?? preferredProviderId),
  ) ?? providerInstances[0] ?? null;
  const preferredProtocol = preferredProvider
    ? PROTOCOLS.find((protocol) => preferredProvider.protocols[protocol.id])?.id
      ?? "openai-responses"
    : "openai-responses";
  const [draft, setDraft] = useState<ModelProfile>(() => initial ?? {
    ...emptyProfile(),
    provider_id: preferredProvider?.id,
    provider: preferredProvider?.provider ?? "",
    protocol: preferredProtocol,
    base_url: preferredProvider?.base_url ?? "",
    env_key: preferredProvider?.env_key,
  });
  const [busy, setBusy] = useState(false);
  const [modelDiscoveryByProvider, setModelDiscoveryByProvider] = useState<
    Record<string, ProviderModelDiscoveryState>
  >({});
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [modelDiscoveryQuery, setModelDiscoveryQuery] = useState("");
  const modelListId = useId();
  const modelDiscoveryRequests = useRef<Record<string, number>>({});
  const modelDiscoveryRequested = useRef(new Set<string>());
  const autoContextWindow = useRef<number | null>(null);
  const activeProviderId = useRef<string | null>(draft.provider_id ?? null);
  const handledInitialProvider = useRef(false);
  const previousProviderId = useRef<string | undefined>(draft.provider_id);
  const mounted = useRef(true);
  const toast = useToast();
  const providerInstance = providerInstances.find(
    (provider) => provider.id === draft.provider_id,
  ) ?? null;
  const modelDiscoveryAvailable = providerInstance !== null;
  const availableProtocols = providerInstance
    ? PROTOCOLS.filter((protocol) => Boolean(providerInstance.protocols[protocol.id]))
    : [];
  const requestUrl = providerInstance
    ? fullRequestUrl(
        providerInstance.base_url,
        providerInstance.protocols[draft.protocol]?.endpoint_path ?? "",
      )
    : "";
  activeProviderId.current = providerInstance?.id ?? null;

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const loadProviderModels = useCallback(async (providerId: string, force = false) => {
    const provider = providerInstances.find((candidate) => candidate.id === providerId);
    if (!provider) return;
    if (!force && modelDiscoveryRequested.current.has(providerId)) return;
    modelDiscoveryRequested.current.add(providerId);
    const requestId = (modelDiscoveryRequests.current[providerId] ?? 0) + 1;
    modelDiscoveryRequests.current[providerId] = requestId;
    if (activeProviderId.current === providerId) setModelDiscoveryQuery("");
    setModelDiscoveryByProvider((current) => ({
      ...current,
      [providerId]: {
        status: "loading",
        models: current[providerId]?.models ?? [],
      },
    }));
    try {
      const models = await discoverProviderModels(providerId);
      if (!mounted.current || modelDiscoveryRequests.current[providerId] !== requestId) return;
      setModelDiscoveryByProvider((current) => ({
        ...current,
        [providerId]: { status: "success", models },
      }));
      if (activeProviderId.current === providerId) setModelPickerOpen(true);
    } catch (error) {
      if (!mounted.current || modelDiscoveryRequests.current[providerId] !== requestId) return;
      setModelDiscoveryByProvider((current) => ({
        ...current,
        [providerId]: {
          status: "error",
          models: current[providerId]?.models ?? [],
          error: formatError(error),
        },
      }));
    }
  }, [providerInstances]);

  useEffect(() => {
    const providerId = providerInstance?.id;
    if (!providerId) return;
    const changed = previousProviderId.current !== providerId;
    previousProviderId.current = providerId;
    if (!handledInitialProvider.current) {
      handledInitialProvider.current = true;
      if (initial) return;
    } else if (!changed) {
      return;
    }
    void loadProviderModels(providerId);
  }, [initial, loadProviderModels, providerInstance]);

  const activeModelDiscovery = providerInstance
    ? modelDiscoveryByProvider[providerInstance.id]
    : undefined;
  const modelQuery = modelDiscoveryQuery.trim().toLocaleLowerCase();
  const matchingProviderModels = (activeModelDiscovery?.models ?? []).filter((model) =>
    !modelQuery
      || model.id.toLocaleLowerCase().includes(modelQuery)
      || model.name?.toLocaleLowerCase().includes(modelQuery)
  );
  const visibleProviderModels = matchingProviderModels.slice(0, MAX_VISIBLE_DISCOVERY_MODELS);

  const selectProvider = (providerId: string) => {
    const provider = providerInstances.find((candidate) => candidate.id === providerId);
    if (!provider) return;
    const protocol = provider.protocols[draft.protocol]
      ? draft.protocol
      : PROTOCOLS.find((candidate) => provider.protocols[candidate.id])?.id
        ?? draft.protocol;
    activeProviderId.current = provider.id;
    setModelPickerOpen(false);
    setModelDiscoveryQuery("");
    const previousAutoContextWindow = autoContextWindow.current;
    autoContextWindow.current = null;
    setDraft((current) => ({
      ...current,
      provider_id: provider.id,
      provider: provider.provider,
      protocol,
      base_url: provider.base_url,
      env_key: provider.env_key,
      context_window: previousAutoContextWindow !== null
        && current.context_window === previousAutoContextWindow
        ? undefined
        : current.context_window,
    }));
  };

  const valid = Boolean(
    providerInstance
      && providerInstance.protocols[draft.protocol]
      && requestUrl
      && draft.model.trim()
      && !busy,
  );

  const save = async () => {
    if (!valid) return;
    setBusy(true);
    try {
      await onReview({
        ...draft,
        id: initial?.id ?? "",
        name: draft.name.trim(),
        model: draft.model.trim(),
      });
    } catch (error) {
      toast.show({ kind: "error", msg: t("models.saveFailed", { error: formatError(error) }) });
    } finally {
      setBusy(false);
    }
  };

  const footer = (
    <>
      <button type="button" className="btn-ghost" disabled={busy} onClick={onClose}>{t("common.cancel")}</button>
      <button type="button" className="btn-primary" disabled={!valid} onClick={() => void save()}>
        {initial
          ? busy ? t("common.saving") : t("common.save")
          : busy ? t("models.addingAction") : t("models.addAction")}
      </button>
    </>
  );
  const form = (
    <div className="mux-model-form">
      <div className="mux-model-form-grid">
        <label>
          <span>{t("models.optionalName")}</span>
          <input
            autoFocus
            className="mux-model-field"
            value={draft.name}
            onChange={(event) => setDraft({ ...draft, name: event.target.value })}
            placeholder={t("models.generatedName")}
          />
        </label>
        <div className="mux-model-form-field">
          <span>{t("models.provider")}</span>
          {providerSelectionLocked && providerInstance ? (
            <input
              aria-label={t("models.provider")}
              className="mux-model-field"
              readOnly
              value={providerInstance.name}
            />
          ) : (
            <>
              <FormSelect
                ariaLabel={t("models.provider")}
                value={draft.provider_id ?? ""}
                placeholder={t("models.providerPlaceholder")}
                options={providerInstances.map((provider) => ({
                  value: provider.id,
                  label: provider.name,
                }))}
                onChange={selectProvider}
              />
              {providerInstances.length === 0 && (
                <div className="mux-model-provider-required">
                  <small>{t("models.providerRequired")}</small>
                  {onAddProvider && (
                    <button type="button" className="btn-secondary" onClick={onAddProvider}>
                      <PlusIcon className="w-3.5 h-3.5" />
                      {t("models.addProvider")}
                    </button>
                  )}
                </div>
              )}
            </>
          )}
        </div>
      </div>

      <div className="mux-model-form-grid">
        <div className="mux-model-form-field">
          <span>{t("models.protocol")}</span>
          <FormSelect
            ariaLabel={t("models.protocol")}
            value={draft.protocol}
            options={availableProtocols.map((protocol) => ({ value: protocol.id, label: protocol.label }))}
            onChange={(protocol) => {
              const nextProtocol = protocol as ModelProtocol;
              setDraft({
                ...draft,
                protocol: nextProtocol,
                base_url: providerInstance?.base_url ?? "",
              });
            }}
          />
        </div>
        <div className="mux-model-form-field mux-provider-model-picker mux-model-form-wide">
          <span>{t("models.modelId")}</span>
          <div className="mux-provider-model-input">
            <input
              aria-autocomplete={modelDiscoveryAvailable ? "list" : undefined}
              aria-controls={modelDiscoveryAvailable ? modelListId : undefined}
              aria-expanded={modelDiscoveryAvailable ? modelPickerOpen : undefined}
              aria-label={t("models.modelId")}
              className="mux-model-field"
              role={modelDiscoveryAvailable ? "combobox" : undefined}
              value={draft.model}
              onChange={(event) => {
                const model = event.currentTarget.value;
                const previousAutoContextWindow = autoContextWindow.current;
                autoContextWindow.current = null;
                setDraft((current) => ({
                  ...current,
                  model,
                  context_window: previousAutoContextWindow !== null
                    && current.context_window === previousAutoContextWindow
                    ? undefined
                    : current.context_window,
                }));
                setModelDiscoveryQuery(model);
                if (activeModelDiscovery?.status === "success") setModelPickerOpen(true);
              }}
              onFocus={() => {
                if (activeModelDiscovery?.status === "success") setModelPickerOpen(true);
              }}
              placeholder="model-name"
              spellCheck={false}
            />
            {providerInstance && (
              <button
                type="button"
                className="mux-provider-model-refresh"
                aria-label={t("models.refreshModelCatalog")}
                title={t("models.refreshModelCatalog")}
                aria-busy={activeModelDiscovery?.status === "loading"}
                disabled={activeModelDiscovery?.status === "loading"}
                onClick={() => void loadProviderModels(providerInstance.id, true)}
              >
                <RefreshIcon className="w-4 h-4" />
              </button>
            )}
          </div>
          {providerInstance
            && activeModelDiscovery
            && activeModelDiscovery.status !== "success" && (
            <div
              className="mux-provider-model-status"
              data-status={activeModelDiscovery.status}
              role="status"
            >
              {activeModelDiscovery.status === "loading" && t("models.loadingModelCatalog")}
              {activeModelDiscovery.status === "error" && t("models.modelCatalogError", {
                error: activeModelDiscovery.error,
              })}
            </div>
          )}
          {providerInstance
            && modelPickerOpen
            && activeModelDiscovery?.status === "success" && (
            <div
              id={modelListId}
              className="mux-provider-model-options"
              role="listbox"
              aria-label={t("models.modelCatalogSuggestions")}
            >
              {visibleProviderModels.map((model) => (
                <button
                  type="button"
                  role="option"
                  aria-selected={draft.model === model.id}
                  className="mux-provider-model-option"
                  key={model.id}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => {
                    const previousAutoContextWindow = autoContextWindow.current;
                    const nextAutoContextWindow = model.context_length && model.context_length > 0
                      ? model.context_length
                      : null;
                    autoContextWindow.current = nextAutoContextWindow;
                    setDraft((current) => ({
                      ...current,
                      model: model.id,
                      context_window: nextAutoContextWindow
                        ?? (previousAutoContextWindow !== null
                          && current.context_window === previousAutoContextWindow
                          ? undefined
                          : current.context_window),
                    }));
                    setModelDiscoveryQuery("");
                    setModelPickerOpen(false);
                  }}
                >
                  <span>
                    <strong>{model.name || model.id}</strong>
                    {model.name && <code>{model.id}</code>}
                  </span>
                  {model.context_length && <small>{formatTokens(model.context_length)}</small>}
                </button>
              ))}
              {matchingProviderModels.length === 0 && (
                <div className="mux-provider-model-empty">{t("models.noModelCatalogMatches")}</div>
              )}
              {matchingProviderModels.length > visibleProviderModels.length && (
                <div className="mux-provider-model-limit">
                  {t("models.modelCatalogShowing", { count: visibleProviderModels.length })}
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      <label className="mux-model-form-wide">
        <span>{t("models.fullRequestUrl")}</span>
        <input
          className="mux-model-field mux-model-url-preview"
          value={requestUrl}
          placeholder={t("models.fullRequestUrlUnavailable")}
          readOnly
        />
      </label>

      <div className="mux-model-form-grid">
        <label>
          <span>{t("models.contextWindow")}</span>
          <input
            type="number"
            min={1}
            className="mux-model-field"
            value={draft.context_window ?? ""}
            onChange={(event) => {
              const value = event.currentTarget.value;
              autoContextWindow.current = null;
              setDraft((current) => ({
                ...current,
                context_window: value ? Number(value) : undefined,
              }));
            }}
          />
        </label>
        <label>
          <span>{t("models.maxOutput")}</span>
          <input
            type="number"
            min={1}
            className="mux-model-field"
            value={draft.max_output_tokens ?? ""}
            onChange={(event) => setDraft({
              ...draft,
              max_output_tokens: event.target.value ? Number(event.target.value) : undefined,
            })}
          />
        </label>
      </div>

      <div className="mux-model-form-field mux-model-form-wide">
        <span>{t("models.reasoningMode")}</span>
        <FormSelect
          ariaLabel={t("models.reasoningMode")}
          value={draft.reasoning === undefined ? "auto" : draft.reasoning ? "on" : "off"}
          options={[
            { value: "auto", label: t("models.reasoningAuto") },
            { value: "on", label: t("models.reasoningOn") },
            { value: "off", label: t("models.reasoningOff") },
          ]}
          onChange={(value) => setDraft({
            ...draft,
            reasoning: value === "auto" ? undefined : value === "on",
          })}
        />
      </div>
    </div>
  );

  if (presentation === "inspector" && initial) {
    return (
      <ResourceInspector
        title={t("models.editTitle")}
        avatar={<Avatar seed={initial.name} kind="model" size={40} />}
        subtitle={t("models.modelRelationshipSubtitle")}
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
      className="mux-dialog-model-editor"
      kind="editor"
      size="md"
      title={initial ? t("models.editTitle") : t("models.createTitle")}
      subtitle={t("models.modelRelationshipSubtitle")}
      busy={busy}
      onClose={onClose}
      footerEnd={footer}
    >
      {form}
    </DialogShell>
  );
}

function ModelProviderDialog({
  initial,
  providerTemplate,
  providers,
  onClose,
  onReview,
}: {
  initial: ModelProviderInstanceView | null;
  providerTemplate: ModelProviderView | null;
  providers: ModelProviderView[];
  onClose: () => void;
  onReview: (provider: ModelProviderConfig, credential?: string) => Promise<void>;
}) {
  const { t } = useTranslation();
  const toast = useToast();
  const initialProviderType = initial?.provider ?? providerTemplate?.id ?? "";
  const template = providerTemplate
    ?? providers.find((provider) => provider.id === initialProviderType)
    ?? null;
  const templateConnection = providerTemplateConnection(template);
  const initialProtocols = initial
    ? Object.fromEntries(
        Object.entries(initial.protocols)
          .map(([protocol, config]) => [protocol, { ...config! }]),
      ) as ModelProviderConfig["protocols"]
    : templateConnection.protocols;
  const initialSource = initial?.api_key_source
    ?? (initial?.env_key ? { kind: "env", name: initial.env_key } satisfies ApiKeySource : undefined)
    ?? (initial?.credential_saved ? { kind: "mux-store" } satisfies ApiKeySource : undefined);
  const initialAuthRequirement: ModelProviderConfig["auth_requirement"] = initial?.auth_requirement
    ?? (["ollama", "lm-studio", "vllm"].includes(initialProviderType) ? "none" : initialProviderType === "custom" ? "optional" : "required");
  const [draft, setDraft] = useState<ModelProviderConfig>({
    id: initial?.id ?? "",
    name: initial?.name ?? providerTemplate?.name ?? "",
    provider: initialProviderType,
    base_url: initial?.base_url ?? templateConnection.base_url,
    model_catalog_url: initial?.model_catalog_url,
    protocols: initialProtocols,
    auth_requirement: initialAuthRequirement,
    api_key_source: initialSource,
  });
  const [protocolPaths, setProtocolPaths] = useState<Record<ModelProtocol, string>>(
    Object.fromEntries(
      PROTOCOLS.map(({ id }) => [
        id,
        initialProtocols[id]?.endpoint_path ?? providerTemplatePath(template, id),
      ]),
    ) as Record<ModelProtocol, string>,
  );
  const [selectedProtocol, setSelectedProtocol] = useState<ModelProtocol>(
    PROTOCOLS.find(({ id }) => Boolean(initialProtocols[id]))?.id ?? "openai-responses",
  );
  const [credential, setCredential] = useState("");
  const [credentialDirty, setCredentialDirty] = useState(false);
  const [credentialLoading, setCredentialLoading] = useState(false);
  const [credentialVisible, setCredentialVisible] = useState(false);
  const [clearCredential, setClearCredential] = useState(false);
  const [busy, setBusy] = useState(false);
  const enabledProtocols = PROTOCOLS.filter(({ id }) => Boolean(draft.protocols[id]));
  const selectedProtocolInfo = PROTOCOLS.find(({ id }) => id === selectedProtocol) ?? PROTOCOLS[0];
  const selectedProtocolPath = protocolPaths[selectedProtocol];
  const selectedProtocolPreview = fullRequestUrl(draft.base_url, selectedProtocolPath);
  const selectedProtocolEnabled = Boolean(draft.protocols[selectedProtocol]);
  const normalizedBaseUrl = normalizeBaseUrl(draft.base_url);
  const normalizedModelCatalogUrl = draft.model_catalog_url
    ? normalizeModelCatalogUrl(draft.model_catalog_url) ?? undefined
    : undefined;
  const protocolsValid = enabledProtocols.length > 0
    && enabledProtocols.every(({ id }) => {
      const path = draft.protocols[id]?.endpoint_path ?? "";
      return Boolean(normalizeEndpointPath(path) && fullRequestUrl(draft.base_url, path));
    });
  const enteredCredential = Boolean(credential.trim());
  const preservedCredential = Boolean(initial?.credential_saved && !clearCredential);
  const preservesLegacySource = Boolean(initialSource && initialSource.kind !== "mux-store");
  const authWithoutCredential: ModelProviderConfig["auth_requirement"] =
    ["ollama", "lm-studio", "vllm"].includes(initialProviderType)
      ? "none"
      : initialProviderType === "custom"
        ? "optional"
        : "required";
  const sourceValid = authWithoutCredential !== "required"
    || enteredCredential
    || preservedCredential
    || preservesLegacySource;
  const valid = Boolean(
    draft.name.trim()
      && draft.provider.trim()
      && normalizedBaseUrl
      && (!draft.model_catalog_url || normalizedModelCatalogUrl)
      && protocolsValid
      && sourceValid
      && !busy
      && !credentialLoading,
  );

  const toggleCredentialVisibility = async () => {
    if (credentialVisible) {
      setCredentialVisible(false);
      return;
    }
    if (!initial?.credential_saved || credential || credentialDirty) {
      setCredentialVisible(true);
      return;
    }

    setCredentialLoading(true);
    try {
      const savedCredential = await revealModelProviderCredential(initial.id);
      setCredential(savedCredential);
      setCredentialDirty(false);
      setCredentialVisible(true);
    } catch (error) {
      toast.show({
        kind: "error",
        msg: t("models.revealApiKeyFailed", { error: formatError(error) }),
      });
    } finally {
      setCredentialLoading(false);
    }
  };

  const save = async () => {
    if (!valid) return;
    setBusy(true);
    try {
      const protocols = Object.fromEntries(
        Object.entries(draft.protocols)
          .filter(([, config]) => config?.endpoint_path.trim())
          .map(([protocol, config]) => [
            protocol,
            { endpoint_path: normalizeEndpointPath(config!.endpoint_path)! },
          ]),
      ) as ModelProviderConfig["protocols"];
      const hasNewCredential = credentialDirty && enteredCredential;
      const apiKeySource = clearCredential
        ? undefined
        : hasNewCredential
          ? { kind: "mux-store" as const }
          : initialSource;
      const authRequirement = hasNewCredential
        ? "required"
        : clearCredential
          ? authWithoutCredential
          : initialSource || preservedCredential
            ? initialAuthRequirement
            : authWithoutCredential;
      await onReview({
        ...draft,
        name: draft.name.trim(),
        provider: draft.provider.trim(),
        base_url: normalizedBaseUrl!,
        model_catalog_url: normalizedModelCatalogUrl,
        protocols,
        auth_requirement: authRequirement,
        api_key_source: authRequirement === "none" ? undefined : apiKeySource,
        env_key: undefined,
      }, authRequirement === "none"
        ? initial?.credential_saved ? "" : undefined
        : clearCredential ? "" : hasNewCredential ? credential : undefined);
    } catch (error) {
      toast.show({ kind: "error", msg: t("models.saveFailed", { error: formatError(error) }) });
    } finally {
      setBusy(false);
    }
  };

  const dialogName = initial?.name || providerTemplate?.name || draft.name || t("models.provider");

  return (
    <DialogShell
      className="mux-dialog-provider-editor"
      kind="editor"
      size="wide"
      borderRadius="10px"
      title={initial
        ? t("models.editProviderNamed", { name: dialogName })
        : t("models.addProviderNamed", { name: dialogName })}
      busy={busy}
      onClose={onClose}
      footerEnd={(
        <>
          <button type="button" className="btn-secondary" disabled={busy} onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button type="button" className="btn-primary" disabled={!valid} onClick={() => void save()}>
            {busy ? t("common.saving") : t("common.save")}
          </button>
        </>
      )}
    >
      <div className="mux-model-form mux-provider-form">
        <div className="mux-provider-basic-grid">
          <label>
            <span>{t("models.providerNameShort")}</span>
            <input
              autoFocus
              className="mux-model-field"
              value={draft.name}
              onChange={(event) => setDraft({ ...draft, name: event.target.value })}
            />
          </label>
          <label>
            <span>{t("models.baseUrl")}</span>
            <input
              aria-label={t("models.baseUrl")}
              className="mux-model-field"
              value={draft.base_url}
              onChange={(event) => setDraft({ ...draft, base_url: event.currentTarget.value })}
              placeholder="https://gateway.example.com/api/v2"
              spellCheck={false}
            />
            {draft.base_url && !normalizedBaseUrl && <small>{t("models.invalidBaseUrl")}</small>}
          </label>
          <label className="mux-provider-model-catalog-field">
            <span>{t("models.modelCatalogUrl")}</span>
            <input
              aria-label={t("models.modelCatalogUrl")}
              className="mux-model-field"
              value={draft.model_catalog_url ?? ""}
              onChange={(event) => setDraft({
                ...draft,
                model_catalog_url: event.currentTarget.value || undefined,
              })}
              placeholder="https://gateway.example.com/v1/models"
              spellCheck={false}
            />
            {draft.model_catalog_url && !normalizedModelCatalogUrl && (
              <small>{t("models.invalidModelCatalogUrl")}</small>
            )}
          </label>
        </div>

        {initialAuthRequirement !== "none" && (
        <section className="mux-provider-form-section mux-provider-credential" aria-label={t("models.apiKey")}>
          <div className="mux-provider-section-head">
            <strong>{t("models.apiKey")}</strong>
            <small>{t("models.credentialHelp")}</small>
          </div>

          <div className="mux-model-form-field mux-provider-credential-field">
            <div className="mux-provider-credential-input" data-mode="mux-store">
                <input
                  type={!credentialVisible ? "password" : "text"}
                  autoComplete="new-password"
                  aria-label={t("models.apiKey")}
                  value={credential}
                  disabled={clearCredential || credentialLoading}
                  onChange={(event) => {
                    setCredential(event.target.value);
                    setCredentialDirty(true);
                  }}
                  placeholder={initial?.credential_saved
                    ? t("models.keepCredential")
                    : preservesLegacySource
                      ? t("models.legacyCredentialPreserved")
                      : t("models.optionalCredential")}
                />
                  <button
                    type="button"
                    className="mux-provider-icon-button"
                    aria-label={credentialVisible ? t("models.hideApiKey") : t("models.showApiKey")}
                    title={credentialVisible ? t("models.hideApiKey") : t("models.showApiKey")}
                    disabled={credentialLoading || clearCredential}
                    aria-busy={credentialLoading}
                    onClick={() => void toggleCredentialVisibility()}
                  >
                    {credentialVisible
                      ? <EyeOffIcon className="w-4 h-4" />
                      : <EyeIcon className="w-4 h-4" />}
                  </button>
            </div>
          </div>

          {initial?.credential_saved && initialSource?.kind === "mux-store" && (
            <label className="mux-model-check mux-provider-credential-clear">
              <input
                type="checkbox"
                checked={clearCredential}
                onChange={(event) => {
                  setClearCredential(event.target.checked);
                  if (event.target.checked) setCredentialVisible(false);
                }}
              />
              {t("models.clearCredential")}
            </label>
          )}
        </section>
        )}

        <section className="mux-provider-form-section mux-provider-protocols" aria-label={t("models.supportedProtocols")}>
          <div className="mux-provider-section-head">
            <strong>{t("models.protocolsShort")}</strong>
            {enabledProtocols.length === 0 && (
              <small className="mux-provider-protocol-error" role="status">
                {t("models.protocolRequired")}
              </small>
            )}
          </div>
          <div className="mux-provider-protocol-list">
            {PROTOCOLS.map((protocol) => {
              const enabled = Boolean(draft.protocols[protocol.id]);
              const path = protocolPaths[protocol.id];
              const pathSummary = normalizeEndpointPath(path) ?? "—";
              const selected = selectedProtocol === protocol.id;
              return (
                <article
                  className="mux-provider-protocol"
                  data-enabled={enabled ? "true" : undefined}
                  data-selected={selected ? "true" : undefined}
                  key={protocol.id}
                >
                  <button
                    type="button"
                    className="mux-provider-protocol-trigger"
                    aria-pressed={selected}
                    onClick={() => setSelectedProtocol(protocol.id)}
                  >
                    <span className="mux-provider-protocol-signal">
                      <span className="mux-model-protocol-dot" data-protocol={protocol.id} />
                    </span>
                    <span className="mux-provider-protocol-copy">
                      <strong>{protocol.label}</strong>
                      <code className="mux-provider-protocol-path" title={pathSummary}>{pathSummary}</code>
                    </span>
                  </button>
                  <label className="mux-provider-protocol-toggle">
                    <input
                      aria-label={protocol.label}
                      className="mux-provider-protocol-switch-input"
                      type="checkbox"
                      role="switch"
                      checked={enabled}
                      onChange={(event) => {
                        const checked = event.target.checked;
                        setSelectedProtocol(protocol.id);
                        setDraft((current) => {
                          const protocols = { ...current.protocols };
                          if (checked) protocols[protocol.id] = { endpoint_path: protocolPaths[protocol.id] };
                          else delete protocols[protocol.id];
                          return { ...current, protocols };
                        });
                      }}
                    />
                    <span className="mux-provider-protocol-switch" aria-hidden="true" />
                  </label>
                </article>
              );
            })}
          </div>
          <div
            className="mux-provider-protocol-editor"
            data-enabled={selectedProtocolEnabled ? "true" : undefined}
          >
            <div
              className="mux-provider-route-builder"
              data-enabled={selectedProtocolEnabled ? "true" : undefined}
              data-invalid={selectedProtocolPath && !normalizeEndpointPath(selectedProtocolPath) ? "true" : undefined}
            >
              <input
                aria-label={`${selectedProtocolInfo.label} ${t("models.endpointPath")}`}
                className="mux-provider-route-path"
                value={selectedProtocolPath}
                onChange={(event) => {
                  const endpointPath = event.currentTarget.value;
                  setProtocolPaths((current) => ({ ...current, [selectedProtocol]: endpointPath }));
                  if (selectedProtocolEnabled) {
                    setDraft((current) => ({
                      ...current,
                      protocols: {
                        ...current.protocols,
                        [selectedProtocol]: { endpoint_path: endpointPath },
                      },
                    }));
                  }
                }}
                placeholder={DEFAULT_ENDPOINT_PATHS[selectedProtocol]}
                spellCheck={false}
              />
              <output
                aria-label={t("models.fullRequestUrl")}
                className="mux-provider-route-preview"
                data-empty={!selectedProtocolPreview ? "true" : undefined}
                title={selectedProtocolPreview || undefined}
              >
                {selectedProtocolPreview || "—"}
              </output>
            </div>
            {selectedProtocolEnabled && selectedProtocolPath && !normalizeEndpointPath(selectedProtocolPath) && (
              <small className="mux-provider-route-error">{t("models.invalidEndpointPath")}</small>
            )}
          </div>
        </section>
      </div>
    </DialogShell>
  );
}
