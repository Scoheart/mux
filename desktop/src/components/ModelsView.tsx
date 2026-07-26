import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  inferModelProvider,
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
import { ResourceCard, ResourceKindIcon } from "./ResourceCard";
import { ResourceState } from "./ResourceState";
import { DialogShell } from "./DialogShell";
import { AssetOperationReviewDialog } from "./AssetOperationReviewDialog";
import { FormSelect } from "./FormSelect";
import {
  EditIcon,
  LayersIcon,
  NetworkIcon,
  PlusIcon,
  TrashIcon,
} from "./icons";
import { useToast } from "./Toast";
import {
  InspectorField,
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
});

function protocolLabel(protocol: ModelProtocol) {
  return PROTOCOLS.find((item) => item.id === protocol)?.label ?? protocol;
}

function providerLabel(providers: ModelProviderView[], provider: string) {
  return providers.find((item) => item.id === provider)?.name ?? (provider || "Custom Provider");
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
  const [providerInstances, setProviderInstances] = useState<ModelProviderInstanceView[]>([]);
  const [providerFilter, setProviderFilter] = useState<string | null>(null);
  const [creatingForProviderId, setCreatingForProviderId] = useState<string | null>(null);
  const [editingProvider, setEditingProvider] = useState<ModelProviderInstanceView | null>(null);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [editing, setEditing] = useState<ModelProfileView | null | undefined>(undefined);
  const [loading, setLoading] = useState(true);
  const [readError, setReadError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [assetFilter, setAssetFilter] = useState<ModelAssetFilter>("all");
  const [protocolFilter, setProtocolFilter] = useState<ModelProtocol | null>(null);
  const [modelsDevByProfileId, setModelsDevByProfileId] = useState<Record<string, ModelsDevMetadata>>({});
  const toast = useToast();
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
        toast.show({ kind: "error", msg: t("models.readFailed", { error: message }) });
      })
      .finally(() => setLoading(false));
  }, [refresh, toast]);

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
    const scoped = profiles.filter((profile) =>
      (!protocolFilter || profile.protocol === protocolFilter)
    );
    return {
      all: scoped.length,
      credential: scoped.filter((profile) => profile.credential_saved).length,
      reasoning: scoped.filter((profile) =>
        profile.reasoning ?? modelsDevByProfileId[profile.id]?.reasoning ?? false
      ).length,
    };
  }, [modelsDevByProfileId, profiles, protocolFilter]);

  const filteredProfiles = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    return profiles.filter((profile) => {
      if (providerFilter && profile.provider_id !== providerFilter) return false;
      if (protocolFilter && profile.protocol !== protocolFilter) return false;
      if (assetFilter === "credential" && !profile.credential_saved) return false;
      if (
        assetFilter === "reasoning"
        && !(profile.reasoning ?? modelsDevByProfileId[profile.id]?.reasoning ?? false)
      ) return false;
      if (!needle) return true;
      return [
        profile.name,
        profile.id,
        profile.model,
        profile.base_url,
        profile.provider,
        profile.catalog_key,
        protocolLabel(profile.protocol),
      ]
        .join(" ")
        .toLocaleLowerCase()
        .includes(needle);
    });
  }, [assetFilter, modelsDevByProfileId, profiles, protocolFilter, providerFilter, query]);

  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId) ?? null;
  const selectedProvider = providerInstances.find((provider) => provider.id === providerFilter) ?? null;

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
    setProtocolFilter(null);
    setAssetFilter("all");
    setSelectedProfileId(profile?.id ?? null);
    if (!profile) toast.show({ kind: "error", msg: t("models.notFound", { id: intent.profileId }) });
    onIntentConsumed?.(intent.id);
  }, [intent, loading, onIntentConsumed, profiles, t, toast]);

  const clearSelection = useCallback(() => {
    setSelectedProfileId(null);
    setEditing(undefined);
    setCreatingForProviderId(null);
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
    <>
      <ResourceWorkspace
        title={t("models.title")}
        description={t("models.description")}
        sidebar={
          <WorkspaceSidebar title={t("models.title")} count={profiles.length}>
            <SidebarSection title={t("models.providers") }>
              <SidebarItem
                active={providerFilter === null}
                icon={<NetworkIcon className="w-3.5 h-3.5" />}
                label={t("models.allProviders")}
                count={profiles.length}
                onClick={() => { clearSelection(); setProviderFilter(null); }}
              />
              {providerInstances.map((provider) => (
                <SidebarItem
                  key={provider.id}
                  active={providerFilter === provider.id}
                  icon={<NetworkIcon className="w-3.5 h-3.5" />}
                  label={provider.name}
                  count={provider.model_count}
                  onClick={() => { clearSelection(); setProviderFilter(provider.id); }}
                />
              ))}
            </SidebarSection>
            <SidebarSection title={t("models.protocol")}>
              <SidebarItem
                active={protocolFilter === null}
                icon={<LayersIcon className="w-3.5 h-3.5" />}
                label={t("models.allProtocols")}
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
        searchPlaceholder={t("models.search")}
        filters={
          <ResourceTabs
            label={t("models.asset")}
            value={assetFilter}
            options={[
              { value: "all", label: t("models.all"), count: assetCounts.all },
              { value: "credential", label: t("models.credential"), count: assetCounts.credential },
              { value: "reasoning", label: t("models.reasoningModels"), count: assetCounts.reasoning },
            ]}
            onChange={(value) => { clearSelection(); setAssetFilter(value); }}
          />
        }
        toolbarActions={
          <>
            {migrationCount > 0 && onOpenMigration && (
              <button className="btn-secondary" type="button" onClick={onOpenMigration}>
                {t("models.history", { count: migrationCount })}
              </button>
            )}
            <button className="btn-primary" type="button" disabled={!consumptionState} onClick={() => {
              clearSelection();
              setCreatingForProviderId(providerFilter);
              setEditing(null);
            }}>
              <PlusIcon className="w-4 h-4" />
              {providerFilter ? t("models.addModel") : t("models.createProvider")}
            </button>
          </>
        }
        inspector={selectedProfile && editing?.id === selectedProfile.id ? (
          <ModelProfileDialog
            initial={editing}
            providers={providers}
            providerInstance={providerInstances.find((provider) => provider.id === editing.provider_id) ?? null}
            presentation="inspector"
            onClose={clearSelection}
            onReview={async (profile, credential) => {
              if (!consumptionState) throw new Error(t("models.saveUnavailable"));
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
            providerName={profileProviderName(selectedProfile, providerInstances, providers)}
            metadata={modelsDevByProfileId[selectedProfile.id]}
            onClose={clearSelection}
            onEdit={consumptionState ? () => setEditing(selectedProfile) : undefined}
            onDelete={consumptionState ? () => void planProfileDelete(selectedProfile) : undefined}
          />
        ) : undefined}
        onInspectorClose={clearSelection}
      >
        {selectedProvider && (
          <ProviderBanner
            provider={selectedProvider}
            onEdit={consumptionState ? () => setEditingProvider(selectedProvider) : undefined}
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
                setProtocolFilter(null);
                setAssetFilter("all");
              }}>{t("models.clearFilters")}</button>
            )}
          />
        ) : (
          <ResourceGrid>
            {filteredProfiles.map((profile) => (
              <ModelCard
                key={profile.id}
                profile={profile}
                providerName={profileProviderName(profile, providerInstances, providers)}
                metadata={modelsDevByProfileId[profile.id]}
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
          providerInstance={providerInstances.find((provider) => provider.id === creatingForProviderId) ?? null}
          onClose={() => setEditing(undefined)}
          onReview={async (profile, credential) => {
            if (!consumptionState) throw new Error(t("models.saveUnavailable"));
            await consumptionState.planUpdate({
              domain: "model",
              existing_id: undefined,
              profile,
              credential,
            });
            setEditing(undefined);
            setCreatingForProviderId(null);
          }}
        />
      )}

      {editingProvider && (
        <ModelProviderDialog
          initial={editingProvider}
          providers={providers}
          onClose={() => setEditingProvider(null)}
          onReview={async (provider, credential) => {
            if (!consumptionState) throw new Error(t("models.saveUnavailable"));
            await consumptionState.planUpdate({
              domain: "model-provider",
              provider,
              credential,
            });
            setEditingProvider(null);
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
              msg: kind === "delete-asset" ? t("models.deleted") : t("models.saved"),
            });
          }}
        />
      )}
    </>
  );
}

function ProviderBanner({
  provider,
  onEdit,
}: {
  provider: ModelProviderInstanceView;
  onEdit?: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="mux-model-provider-banner">
      <div className="mux-model-provider-banner-copy">
        <strong>{provider.name}</strong>
        <span>{t("models.providerModelCount", { count: provider.model_count })}</span>
        <Badge tone={provider.credential_saved ? "success" : "neutral"}>
          {provider.credential_saved ? t("models.keychainSaved") : t("models.keychainNotSaved")}
        </Badge>
      </div>
      <button className="btn-secondary" type="button" disabled={!onEdit} onClick={onEdit}>
        <EditIcon className="w-4 h-4" />
        {t("models.editProvider")}
      </button>
    </div>
  );
}

function ModelCard({
  profile,
  providerName,
  metadata,
  selected,
  onOpen,
}: {
  profile: ModelProfileView;
  providerName: string;
  metadata?: ModelsDevMetadata;
  selected: boolean;
  onOpen: () => void;
}) {
  const { t } = useTranslation();
  const displayName = readableModelName(profile, providerName, metadata);
  const contextWindow = profile.context_window ?? metadata?.contextWindow;
  const maxOutputTokens = profile.max_output_tokens ?? metadata?.maxOutputTokens;
  const capabilities = [
    (profile.reasoning ?? metadata?.reasoning ?? false) && t("models.reasoning"),
    metadata?.toolCall && t("models.tools"),
    metadata?.structuredOutput && t("models.structured"),
    metadata?.modalities?.some((modality) => modality !== "text") && t("models.multimodal"),
  ].filter((item): item is string => Boolean(item)).slice(0, 3);
  return (
    <ResourceCard
      className="mux-model-card"
      selected={selected}
      ariaLabel={t("models.openDetails", { name: profile.name })}
      onOpen={onOpen}
      identity={
        <>
          <ResourceKindIcon kind="model" seed={displayName} />
          <div className="mux-resource-card-copy">
            <div className="mux-resource-card-heading">
              <strong title={displayName}>{displayName}</strong>
            </div>
            <div className="mux-model-card-subtitle">
              <code className="mux-resource-card-code" title={profile.model}>{profile.model}</code>
              <span className="mux-model-card-provider">{providerName}</span>
            </div>
          </div>
        </>
      }
      configuration={metadata?.description ? (
        <p className="mux-model-card-description" title={metadata.description}>{metadata.description}</p>
      ) : undefined}
      state={(contextWindow || maxOutputTokens || metadata?.inputCost != null || metadata?.outputCost != null || capabilities.length > 0) ? (
        <>
          {contextWindow && <span className="mux-model-card-metric">{t("models.contextMetric", { value: formatTokens(contextWindow) })}</span>}
          {maxOutputTokens && <span className="mux-model-card-metric">{t("models.outputMetric", { value: formatTokens(maxOutputTokens) })}</span>}
          {metadata?.inputCost != null && (
            <span className="mux-model-card-metric">{t("models.inputMetric", { value: formatCatalogCost(metadata.inputCost) })}</span>
          )}
          {metadata?.outputCost != null && (
            <span className="mux-model-card-metric">{t("models.outputPriceMetric", { value: formatCatalogCost(metadata.outputCost) })}</span>
          )}
          {capabilities.map((capability) => (
            <span className="mux-model-card-capability" key={capability}>{capability}</span>
          ))}
        </>
      ) : undefined}
      impact={
        <div className="mux-model-card-footer-line">
          <span className="mux-model-card-protocol">{protocolLabel(profile.protocol)}</span>
        </div>
      }
    />
  );
}

function ModelInspector({
  profile,
  providerName,
  metadata,
  onClose,
  onEdit,
  onDelete,
}: {
  profile: ModelProfileView;
  providerName: string;
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
        <InspectorField label={t("models.baseUrl")} mono>{profile.base_url}</InspectorField>
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

function ModelProfileDialog({
  initial,
  providers,
  providerInstance,
  onClose,
  onReview,
  presentation = "dialog",
}: {
  initial: ModelProfileView | null;
  providers: ModelProviderView[];
  providerInstance: ModelProviderInstanceView | null;
  onClose: () => void;
  onReview: (profile: ModelProfile, credential?: string) => Promise<void>;
  presentation?: "dialog" | "inspector";
}) {
  const { t } = useTranslation();
  const providerProtocol = providerInstance
    ? PROTOCOLS.find((protocol) => providerInstance.endpoints[protocol.id])?.id
      ?? "openai-responses"
    : null;
  const [draft, setDraft] = useState<ModelProfile>(() => initial ?? (providerInstance ? {
    ...emptyProfile(),
    provider_id: providerInstance.id,
    provider: providerInstance.provider,
    protocol: providerProtocol ?? "openai-responses",
    base_url: providerInstance.endpoints[providerProtocol ?? "openai-responses"] ?? "",
    env_key: providerInstance.env_key,
  } : emptyProfile()));
  const sharedProvider = providerInstance !== null;
  const availableProtocols = sharedProvider
    ? PROTOCOLS.filter((protocol) => Boolean(providerInstance.endpoints[protocol.id]))
    : PROTOCOLS;
  const [credential, setCredential] = useState("");
  const [clearCredential, setClearCredential] = useState(false);
  const [busy, setBusy] = useState(false);
  const baseUrlAutoManaged = useRef(initial === null);
  const providerInferenceSequence = useRef(0);
  const initialProviderInferenceStarted = useRef(false);
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

  const inferProviderFromBaseUrl = async (baseUrl: string) => {
    const sequence = ++providerInferenceSequence.current;
    const inferred = await inferModelProvider(baseUrl);
    if (sequence !== providerInferenceSequence.current) return;

    const known = providers.some(
      (provider) => provider.id !== "custom" && provider.id === inferred,
    );
    if (known) {
      setProviderSelection(inferred);
      setDraft((current) => ({ ...current, provider: inferred }));
      return;
    }

    const customId = customProvider.trim() || "custom";
    setCustomProvider(customId);
    setProviderSelection(CUSTOM_PROVIDER_OPTION);
    setDraft((current) => ({ ...current, provider: customId }));
  };

  useEffect(() => {
    const baseUrl = initial?.base_url.trim();
    if (
      initialProviderInferenceStarted.current
      || initialProviderIsKnown
      || !baseUrl
    ) return;

    initialProviderInferenceStarted.current = true;
    void inferProviderFromBaseUrl(baseUrl);
  }, [initial, initialProviderIsKnown]);

  const selectProvider = (provider: string) => {
    providerInferenceSequence.current += 1;
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
    if (baseUrl.trim() && !sharedProvider) {
      void inferProviderFromBaseUrl(baseUrl.trim());
    } else {
      providerInferenceSequence.current += 1;
    }
  };

  const valid = Boolean(
    (sharedProvider ? providerInstance.endpoints[draft.protocol] : draft.base_url.trim())
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
        base_url: draft.base_url.trim().replace(/\/$/, ""),
        model: draft.model.trim(),
        env_key: draft.env_key?.trim() || undefined,
      }, credentialUpdate);
      setCredential("");
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
        {!sharedProvider && <div className="mux-model-form-field">
          <span>{t("models.provider")}</span>
          <FormSelect
            ariaLabel={t("models.provider")}
            value={providerSelection}
            placeholder={t("models.providerPlaceholder")}
            options={[
              ...providers
                .filter((provider) => provider.id !== "custom")
                .map((provider) => ({ value: provider.id, label: provider.name })),
              { value: CUSTOM_PROVIDER_OPTION, label: t("models.customProvider") },
            ]}
            onChange={selectProvider}
          />
          {providerSelection === CUSTOM_PROVIDER_OPTION && (
            <input
              aria-label={t("models.customProviderId")}
              className="mux-model-field mux-model-custom-provider"
              value={customProvider}
              onChange={(event) => updateCustomProvider(event.target.value)}
              placeholder={t("models.customProviderExample")}
              spellCheck={false}
            />
          )}
        </div>}
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
                base_url: providerInstance?.endpoints[nextProtocol] ?? draft.base_url,
              });
            }}
          />
        </div>
        <div className="mux-model-form-field">
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

      <div className="mux-model-form-grid">
        {(!sharedProvider || !providerInstance?.endpoints[draft.protocol]) && <label>
          <span>{t("models.baseUrl")}</span>
          <input
            aria-label={t("models.baseUrl")}
            className="mux-model-field"
            value={draft.base_url}
            onChange={(event) => updateBaseUrl(event.target.value)}
            placeholder="https://api.example.com/v1"
            spellCheck={false}
          />
        </label>}

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

      {!sharedProvider && <div className="mux-model-form-grid">
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
          <small>{t("models.apiKeyEnvHelp")}</small>
        </label>
      </div>}

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
    </div>
  );

  if (presentation === "inspector" && initial) {
    return (
      <ResourceInspector
        title={t("models.editTitle")}
        avatar={<Avatar seed={initial.name} kind="model" size={40} />}
        subtitle={t("models.keychainSubtitle")}
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
      subtitle={t("models.keychainSubtitle")}
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
  providers,
  onClose,
  onReview,
}: {
  initial: ModelProviderInstanceView;
  providers: ModelProviderView[];
  onClose: () => void;
  onReview: (provider: ModelProviderConfig, credential?: string) => Promise<void>;
}) {
  const { t } = useTranslation();
  const toast = useToast();
  const knownProvider = providers.some(
    (provider) => provider.id !== "custom" && provider.id === initial.provider,
  );
  const [draft, setDraft] = useState<ModelProviderConfig>({
    id: initial.id,
    name: initial.name,
    provider: initial.provider,
    endpoints: { ...initial.endpoints },
    env_key: initial.env_key,
  });
  const [providerSelection, setProviderSelection] = useState(
    knownProvider ? initial.provider : CUSTOM_PROVIDER_OPTION,
  );
  const [credential, setCredential] = useState("");
  const [clearCredential, setClearCredential] = useState(false);
  const [busy, setBusy] = useState(false);
  const endpointCount = Object.values(draft.endpoints).filter((value) => value?.trim()).length;
  const valid = Boolean(
    draft.name.trim()
      && draft.provider.trim()
      && endpointCount > 0
      && !busy,
  );

  const save = async () => {
    if (!valid) return;
    setBusy(true);
    try {
      const endpoints = Object.fromEntries(
        Object.entries(draft.endpoints)
          .filter(([, endpoint]) => endpoint?.trim())
          .map(([protocol, endpoint]) => [protocol, endpoint!.trim().replace(/\/$/, "")]),
      ) as ModelProviderConfig["endpoints"];
      await onReview({
        ...draft,
        name: draft.name.trim(),
        provider: draft.provider.trim(),
        endpoints,
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
      title={t("models.editProvider")}
      subtitle={t("models.providerSharedSubtitle", { count: initial.model_count })}
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
              options={[
                ...providers
                  .filter((provider) => provider.id !== "custom")
                  .map((provider) => ({ value: provider.id, label: provider.name })),
                { value: CUSTOM_PROVIDER_OPTION, label: t("models.customProvider") },
              ]}
              onChange={(value) => {
                setProviderSelection(value);
                if (value !== CUSTOM_PROVIDER_OPTION) {
                  setDraft({ ...draft, provider: value });
                }
              }}
            />
            {providerSelection === CUSTOM_PROVIDER_OPTION && (
              <input
                aria-label={t("models.customProviderId")}
                className="mux-model-field mux-model-custom-provider"
                value={draft.provider}
                onChange={(event) => setDraft({ ...draft, provider: event.target.value })}
                spellCheck={false}
              />
            )}
          </div>
        </div>

        {PROTOCOLS.map((protocol) => (
          <label key={protocol.id}>
            <span>{t("models.providerEndpoint", { protocol: protocol.label })}</span>
            <input
              aria-label={t("models.providerEndpoint", { protocol: protocol.label })}
              className="mux-model-field"
              value={draft.endpoints[protocol.id] ?? ""}
              onChange={(event) => {
                const endpoint = event.currentTarget.value;
                setDraft((current) => ({
                  ...current,
                  endpoints: { ...current.endpoints, [protocol.id]: endpoint },
                }));
              }}
              placeholder="https://api.example.com/v1"
              spellCheck={false}
            />
          </label>
        ))}

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
              placeholder={initial.credential_saved ? t("models.keepCredential") : t("models.optionalCredential")}
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

        {initial.credential_saved && (
          <label className="mux-model-check">
            <input
              type="checkbox"
              checked={clearCredential}
              onChange={(event) => setClearCredential(event.target.checked)}
            />
            {t("models.clearCredential")}
          </label>
        )}
      </div>
    </DialogShell>
  );
}
