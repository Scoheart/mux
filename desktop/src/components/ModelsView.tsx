import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  listModelProfiles,
  listModelProviderInstances,
  listModelProviders,
} from "../lib/api";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import type {
  ModelProfile,
  ModelProfileView,
  ModelProviderConfig,
  ModelProviderInstanceView,
  ModelProviderView,
  ModelProtocol,
  ResourceNavigationIntent,
} from "../lib/types";
import { formatError } from "../lib/format";
import {
  getCachedModelsDevMetadata,
  loadModelsDevMetadata,
  type ModelsDevMetadata,
} from "../lib/modelsDev";
import { Avatar, Badge } from "./ui";
import { ResourceKindIcon } from "./ResourceCard";
import { ResourceState } from "./ResourceState";
import { DialogShell } from "./DialogShell";
import { AssetOperationReviewDialog } from "./AssetOperationReviewDialog";
import { FormSelect } from "./FormSelect";
import { ProviderGlyph } from "./providerIcons";
import {
  EditIcon,
  LayersIcon,
  PlusIcon,
  TrashIcon,
} from "./icons";
import { useToast } from "./Toast";
import {
  InspectorField,
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

const CUSTOM_PROVIDER_OPTION = "__custom__";

const emptyProfile = (): ModelProfile => ({
  id: "",
  name: "",
  provider: "",
  protocol: "openai-responses",
  base_url: "",
  model: "",
});

function protocolLabel(protocol: ModelProtocol) {
  return PROTOCOLS.find((item) => item.id === protocol)?.label ?? protocol;
}

function providerLabel(providers: ModelProviderView[], provider: string) {
  return providers.find((item) => item.id === provider)?.name ?? (provider || "Custom Provider");
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
                onClick={() => { clearSelection(); setProviderFilter(null); }}
              />
            </SidebarSection>
            {providerInstances.length > 0 && (
              <SidebarSection title={t("models.myProviders")}>
                {providerInstances.map((provider) => (
                  <SidebarItem
                    key={provider.id}
                    active={providerFilter === provider.id}
                    icon={<ProviderGlyph id={provider.provider} name={provider.name} size={18} />}
                    label={provider.name}
                    count={provider.model_count}
                    onClick={() => { clearSelection(); setProviderFilter(provider.id); }}
                  />
                ))}
              </SidebarSection>
            )}
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
          <Badge tone={provider.credential_saved ? "success" : "neutral"}>
            {provider.credential_saved ? t("models.keychainSaved") : t("models.keychainNotSaved")}
          </Badge>
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
      <div className="mux-asset-list-header mux-model-list-header" aria-hidden="true">
        <span>{t("models.modelColumn")}</span>
        <span>{t("models.providerColumn")}</span>
        <span>{t("models.protocol")}</span>
        <span>{t("models.context")}</span>
        <span>{t("models.credentialColumn")}</span>
      </div>
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
                <ResourceKindIcon kind="model" seed={displayName} />
                <span className="mux-asset-list-copy">
                  <strong title={displayName}>{displayName}</strong>
                  <code title={profile.model}>{profile.model}</code>
                </span>
              </span>
              <span className="mux-model-list-provider" title={providerName}>{providerName}</span>
              <span className="mux-model-list-protocol">{protocolLabel(profile.protocol)}</span>
              <span className="mux-model-list-context">
                {contextWindow ? formatTokens(contextWindow) : t("common.notSet")}
              </span>
              <span className={profile.credential_saved ? "mux-status-ok" : "mux-status-muted"}>
                {profile.credential_saved ? t("models.keychainSaved") : t("models.keychainNotSaved")}
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
      <section className="mux-model-inspector-fields" aria-label={t("models.detailsFields")}>
        <InspectorField label={t("models.provider")}>{providerName}</InspectorField>
        <InspectorField label={t("models.protocol")}>{protocolLabel(profile.protocol)}</InspectorField>
        {showReasoning && (
          <InspectorField label={t("models.reasoningMode")}>
            {profile.reasoning === undefined
              ? t("models.reasoningAuto")
              : profile.reasoning
                ? t("models.reasoningOn")
                : t("models.reasoningOff")}
          </InspectorField>
        )}
        {metadata?.description && <InspectorField label={t("models.modelDescription")}>{metadata.description}</InspectorField>}
        {contextWindow && <InspectorField label={t("models.context")}>{formatTokens(contextWindow)} tokens</InspectorField>}
        {maxOutputTokens && <InspectorField label={t("models.outputLimit")}>{formatTokens(maxOutputTokens)} tokens</InspectorField>}
        {(metadata?.inputCost != null || metadata?.outputCost != null) && (
          <InspectorField label={t("models.catalogPrice")}>
            {[
              metadata.inputCost != null && t("models.inputMetric", { value: formatCatalogCost(metadata.inputCost) }),
              metadata.outputCost != null && t("models.outputPriceMetric", { value: formatCatalogCost(metadata.outputCost) }),
            ].filter(Boolean).join(" · ")}
          </InspectorField>
        )}
        {capabilities.length > 0 && <InspectorField label={t("models.capabilities")}>{capabilities.join(" · ")}</InspectorField>}
        {metadata?.releaseDate && <InspectorField label={t("models.releaseDate")}>{metadata.releaseDate}</InspectorField>}
        <InspectorField label={t("models.modelId")} mono>{profile.model}</InspectorField>
        <InspectorField label={t("models.fullRequestUrl")} mono>{requestUrl || t("common.notSet")}</InspectorField>
        {profile.env_key && <InspectorField label={t("models.environmentVariable")} mono>{profile.env_key}</InspectorField>}
        <InspectorField label={t("models.apiKey")}>
          <span className={profile.credential_saved ? "mux-status-ok" : "mux-status-muted"}>
            {profile.credential_saved ? t("models.keychainSaved") : t("models.keychainNotSaved")}
          </span>
        </InspectorField>
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
    providers.find((provider) => provider.id === "openrouter")?.id ?? providers[0]?.id ?? "",
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
      kind="picker"
      size="lg"
      title={t("models.providerCatalogTitle")}
      subtitle={t("models.providerCatalogSubtitle")}
      onClose={onClose}
      footerStart={selected ? t("models.providerSelected", { name: selected.name }) : undefined}
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
        <input
          autoFocus
          type="search"
          className="mux-model-field"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("models.searchProviders")}
          aria-label={t("models.searchProviders")}
        />
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
  onClose,
  onReview,
  onAddProvider,
  presentation = "dialog",
}: {
  initial: ModelProfileView | null;
  providerInstances: ModelProviderInstanceView[];
  preferredProviderId?: string | null;
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
  const toast = useToast();
  const providerInstance = providerInstances.find(
    (provider) => provider.id === draft.provider_id,
  ) ?? null;
  const availableProtocols = providerInstance
    ? PROTOCOLS.filter((protocol) => Boolean(providerInstance.protocols[protocol.id]))
    : [];
  const requestUrl = providerInstance
    ? fullRequestUrl(
        providerInstance.base_url,
        providerInstance.protocols[draft.protocol]?.endpoint_path ?? "",
      )
    : "";

  const selectProvider = (providerId: string) => {
    const provider = providerInstances.find((candidate) => candidate.id === providerId);
    if (!provider) return;
    const protocol = provider.protocols[draft.protocol]
      ? draft.protocol
      : PROTOCOLS.find((candidate) => provider.protocols[candidate.id])?.id
        ?? draft.protocol;
    setDraft((current) => ({
      ...current,
      provider_id: provider.id,
      provider: provider.provider,
      protocol,
      base_url: provider.base_url,
      env_key: provider.env_key,
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
        {busy ? t("common.saving") : t("common.save")}
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
        <label>
          <span>{t("models.modelId")}</span>
          <input
            className="mux-model-field"
            value={draft.model}
            onChange={(event) => {
              const model = event.currentTarget.value;
              setDraft((current) => ({ ...current, model }));
            }}
            placeholder="model-name"
            spellCheck={false}
          />
        </label>
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
            onChange={(event) => setDraft({
              ...draft,
              context_window: event.target.value ? Number(event.target.value) : undefined,
            })}
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

      <small>{t("models.modelUsesProviderConnection")}</small>
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
  const knownProvider = providers.some(
    (provider) => provider.id !== "custom" && provider.id === initialProviderType,
  );
  const templateConnection = providerTemplateConnection(providerTemplate);
  const [draft, setDraft] = useState<ModelProviderConfig>({
    id: initial?.id ?? "",
    name: initial?.name ?? providerTemplate?.name ?? "",
    provider: initialProviderType,
    base_url: initial?.base_url ?? templateConnection.base_url,
    protocols: initial
      ? Object.fromEntries(
          Object.entries(initial.protocols)
            .map(([protocol, config]) => [protocol, { ...config! }]),
        )
      : templateConnection.protocols,
    env_key: initial?.env_key,
  });
  const [providerSelection, setProviderSelection] = useState(
    knownProvider ? initialProviderType : CUSTOM_PROVIDER_OPTION,
  );
  const [credential, setCredential] = useState("");
  const [clearCredential, setClearCredential] = useState(false);
  const [busy, setBusy] = useState(false);
  const enabledProtocols = PROTOCOLS.filter(({ id }) => Boolean(draft.protocols[id]));
  const normalizedBaseUrl = normalizeBaseUrl(draft.base_url);
  const protocolsValid = enabledProtocols.length > 0
    && enabledProtocols.every(({ id }) => {
      const path = draft.protocols[id]?.endpoint_path ?? "";
      return Boolean(normalizeEndpointPath(path) && fullRequestUrl(draft.base_url, path));
    });
  const valid = Boolean(
    draft.name.trim()
      && draft.provider.trim()
      && normalizedBaseUrl
      && protocolsValid
      && !busy,
  );

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
      await onReview({
        ...draft,
        name: draft.name.trim(),
        provider: draft.provider.trim(),
        base_url: normalizedBaseUrl!,
        protocols,
        env_key: draft.env_key?.trim() || undefined,
      }, clearCredential ? "" : credential || undefined);
    } catch (error) {
      toast.show({ kind: "error", msg: t("models.saveFailed", { error: formatError(error) }) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <DialogShell
      kind="editor"
      size="md"
      title={initial ? t("models.editProvider") : t("models.createProvider")}
      subtitle={initial
        ? t("models.providerSharedSubtitle", { count: initial.model_count })
        : t("models.providerCreateSubtitle")}
      busy={busy}
      onClose={onClose}
      footerEnd={(
        <>
          <button type="button" className="btn-ghost" disabled={busy} onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button type="button" className="btn-primary" disabled={!valid} onClick={() => void save()}>
            {busy ? t("common.saving") : t("common.save")}
          </button>
        </>
      )}
    >
      <div className="mux-model-form">
        <div className="mux-model-form-grid">
          <label>
            <span>{t("models.providerName")}</span>
            <input
              autoFocus
              className="mux-model-field"
              value={draft.name}
              onChange={(event) => setDraft({ ...draft, name: event.target.value })}
            />
          </label>
          <div className="mux-model-form-field">
            <span>{t("models.providerType")}</span>
            <FormSelect
              ariaLabel={t("models.providerType")}
              value={providerSelection}
              options={initial
                ? [{
                    value: knownProvider ? initial.provider : CUSTOM_PROVIDER_OPTION,
                    label: knownProvider
                      ? providerLabel(providers, initial.provider)
                      : t("models.customProvider"),
                  }]
                : [
                    ...providers
                      .filter((provider) => provider.id !== "custom")
                      .map((provider) => ({ value: provider.id, label: provider.name })),
                    { value: CUSTOM_PROVIDER_OPTION, label: t("models.customProvider") },
              ]}
              onChange={(value) => {
                setProviderSelection(value);
                if (value !== CUSTOM_PROVIDER_OPTION) {
                  const previousProvider = providers.find(
                    (provider) => provider.id === draft.provider,
                  );
                  const nextProvider = providers.find(
                    (provider) => provider.id === value,
                  );
                  const previousConnection = providerTemplateConnection(previousProvider);
                  const usesTemplateConnection = (
                    !draft.base_url.trim()
                    || (
                      normalizeBaseUrl(draft.base_url) === normalizeBaseUrl(previousConnection.base_url)
                      && PROTOCOLS.every(({ id }) =>
                        (draft.protocols[id]?.endpoint_path ?? "")
                          === (previousConnection.protocols[id]?.endpoint_path ?? "")
                      )
                    )
                  );
                  const nextConnection = providerTemplateConnection(nextProvider);
                  setDraft({
                    ...draft,
                    provider: value,
                    ...(usesTemplateConnection ? nextConnection : {}),
                  });
                } else if (!initial) {
                  setDraft((current) => ({ ...current, provider: "custom" }));
                }
              }}
            />
          </div>
        </div>

        <label className="mux-model-form-wide">
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

        <div className="mux-model-form-grid">
          <label>
            <span>{t("models.apiKey")}</span>
            <input
              type="password"
              autoComplete="new-password"
              className="mux-model-field"
              value={credential}
              disabled={clearCredential}
              onChange={(event) => setCredential(event.target.value)}
              placeholder={initial?.credential_saved ? t("models.keepCredential") : t("models.optionalCredential")}
            />
          </label>
          <label>
            <span>{t("models.apiKeyEnv")}</span>
            <input
              className="mux-model-field"
              value={draft.env_key ?? ""}
              onChange={(event) => setDraft({ ...draft, env_key: event.target.value || undefined })}
              placeholder="MY_API_KEY"
              spellCheck={false}
            />
          </label>
        </div>

        {initial?.credential_saved && (
          <label className="mux-model-check">
            <input
              type="checkbox"
              checked={clearCredential}
              onChange={(event) => setClearCredential(event.target.checked)}
            />
            {t("models.clearCredential")}
          </label>
        )}

        <section className="mux-provider-protocols" aria-label={t("models.supportedProtocols")}>
          <div className="mux-provider-protocols-head">
            <div>
              <strong>{t("models.supportedProtocols")}</strong>
              <small>{t("models.supportedProtocolsHelp")}</small>
              {enabledProtocols.length === 0 && (
                <small className="mux-provider-protocol-error" role="status">
                  {t("models.protocolRequired")}
                </small>
              )}
            </div>
            <div
              className="mux-provider-protocol-summary"
              data-empty={enabledProtocols.length === 0 ? "true" : undefined}
              aria-live="polite"
            >
              <span className="mux-provider-protocol-meter" aria-hidden="true">
                {PROTOCOLS.map((protocol) => (
                  <i
                    data-enabled={draft.protocols[protocol.id] ? "true" : undefined}
                    key={protocol.id}
                  />
                ))}
              </span>
              <span>{t("models.protocolCount", { count: enabledProtocols.length })}</span>
            </div>
          </div>
          <div className="mux-provider-protocol-list">
            {PROTOCOLS.map((protocol) => {
              const config = draft.protocols[protocol.id];
              const enabled = Boolean(config);
              const path = config?.endpoint_path ?? "";
              const template = providers.find((provider) => provider.id === draft.provider);
              const pathSummary = enabled
                ? normalizeEndpointPath(path) ?? "—"
                : providerTemplatePath(template, protocol.id);
              const preview = enabled ? fullRequestUrl(draft.base_url, path) : "";
              return (
                <article
                  className="mux-provider-protocol"
                  data-enabled={enabled ? "true" : undefined}
                  key={protocol.id}
                >
                  <label className="mux-provider-protocol-toggle">
                    <span className="mux-model-protocol-dot" data-protocol={protocol.id} />
                    <strong>{protocol.label}</strong>
                    <code className="mux-provider-protocol-path" title={pathSummary}>
                      {pathSummary}
                    </code>
                    <input
                      aria-label={protocol.label}
                      className="mux-provider-protocol-switch-input"
                      type="checkbox"
                      role="switch"
                      checked={enabled}
                      onChange={(event) => {
                        setDraft((current) => {
                          const protocols = { ...current.protocols };
                          if (event.target.checked) {
                            protocols[protocol.id] = {
                              endpoint_path: providerTemplatePath(template, protocol.id),
                            };
                          } else {
                            delete protocols[protocol.id];
                          }
                          return { ...current, protocols };
                        });
                      }}
                    />
                    <span className="mux-provider-protocol-switch" aria-hidden="true" />
                  </label>
                  {enabled && (
                    <div className="mux-provider-protocol-fields">
                      <label>
                        <span>{t("models.endpointPath")}</span>
                        <input
                          aria-label={`${protocol.label} ${t("models.endpointPath")}`}
                          className="mux-model-field"
                          value={path}
                          onChange={(event) => {
                            const endpointPath = event.currentTarget.value;
                            setDraft((current) => ({
                              ...current,
                              protocols: {
                                ...current.protocols,
                                [protocol.id]: { endpoint_path: endpointPath },
                              },
                            }));
                          }}
                          placeholder={DEFAULT_ENDPOINT_PATHS[protocol.id]}
                          spellCheck={false}
                        />
                        {path && !normalizeEndpointPath(path) && (
                          <small>{t("models.invalidEndpointPath")}</small>
                        )}
                      </label>
                      <div className="mux-model-form-field mux-provider-url-field">
                        <span>{t("models.fullRequestUrl")}</span>
                        <output
                          aria-label={t("models.fullRequestUrl")}
                          className="mux-provider-url-output"
                          data-empty={preview ? undefined : "true"}
                          title={preview || undefined}
                        >
                          {preview || t("models.fullRequestUrlUnavailable")}
                        </output>
                      </div>
                      <button
                        type="button"
                        className="btn-ghost mux-provider-path-reset"
                        onClick={() => setDraft((current) => ({
                          ...current,
                          protocols: {
                            ...current.protocols,
                            [protocol.id]: {
                              endpoint_path: providerTemplatePath(template, protocol.id),
                            },
                          },
                        }))}
                      >
                        {t("models.restoreDefaultPath")}
                      </button>
                    </div>
                  )}
                </article>
              );
            })}
          </div>
        </section>
      </div>
    </DialogShell>
  );
}
