import { useEffect, useRef, useState, type ReactNode } from "react";
import type { AssetRef, ConsumptionView, ConvergenceAction } from "../lib/types";
import { assetIdentity } from "../lib/consumption";
import {
  DownloadIcon,
  LinkIcon,
  LinkOffIcon,
  MoreHorizontalIcon,
  PackageIcon,
  PlusIcon,
  RefreshIcon,
  TrashIcon,
} from "./icons";
import { ConsumptionStatus } from "./ConsumptionStatus";
import { Switch } from "./ui";
import { useTranslation } from "react-i18next";

export interface ConsumptionAssetPresentation {
  name: string;
  description?: string;
  icon?: ReactNode;
  meta?: ReactNode;
}

function ConvergenceActionIcon({ action }: { action: ConvergenceAction }) {
  if (action === "adopt-observed") return <DownloadIcon className="w-3.5 h-3.5" />;
  if (action === "restore-desired") return <RefreshIcon className="w-3.5 h-3.5" />;
  return <LinkOffIcon className="w-3.5 h-3.5" />;
}

function ConvergenceActions({
  actions,
  disabled,
  onAction,
}: {
  actions: ConvergenceAction[];
  disabled: boolean;
  onAction(action: ConvergenceAction): void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLSpanElement>(null);
  const label = (action: ConvergenceAction) => action === "adopt-observed"
    ? t("observations.actions.adopt")
    : action === "restore-desired"
      ? t("observations.actions.restore")
      : t("observations.actions.detach");
  const primary = actions.includes("restore-desired")
    ? "restore-desired"
    : actions.includes("adopt-observed")
      ? "adopt-observed"
      : actions[0];
  const secondary = actions.filter((action) => action !== primary);

  useEffect(() => {
    if (!open) return;
    const closeOutside = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("pointerdown", closeOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [open]);

  if (!primary) return null;
  return (
    <span className="mux-convergence-controls" ref={rootRef}>
      <button
        type="button"
        className="mux-convergence-action"
        data-primary="true"
        data-action={primary}
        aria-label={label(primary)}
        title={label(primary)}
        disabled={disabled}
        onClick={() => onAction(primary)}
      >
        <ConvergenceActionIcon action={primary} />
      </button>
      {secondary.length > 0 && (
        <span className="mux-convergence-more">
          <button
            type="button"
            className="mux-convergence-action"
            aria-label={t("observations.actions.more")}
            title={t("observations.actions.more")}
            aria-haspopup="menu"
            aria-expanded={open}
            disabled={disabled}
            onClick={() => setOpen((current) => !current)}
          >
            <MoreHorizontalIcon className="w-4 h-4" />
          </button>
          {open && (
            <span className="mux-convergence-menu" role="menu">
              {secondary.map((action) => (
                <button
                  key={action}
                  type="button"
                  role="menuitem"
                  data-action={action}
                  onClick={() => {
                    setOpen(false);
                    onAction(action);
                  }}
                >
                  <ConvergenceActionIcon action={action} />
                  <span>{label(action)}</span>
                </button>
              ))}
            </span>
          )}
        </span>
      )}
    </span>
  );
}

export function AgentConsumptionPanel({
  domain,
  title,
  description,
  manageLabel,
  rows,
  external,
  externalMode = "summary",
  externalTitle = "外部配置",
  externalDescription = "尚未由 MUX 管理",
  present,
  onManage,
  manageIcon = <PlusIcon className="w-3.5 h-3.5" />,
  onOpenAsset,
  onEnabledChange,
  enabledChangeDisabled,
  toggleKind = "enabled",
  renderAction,
  onRemove,
  onConverge,
  convergenceDisabled = false,
  removeLabel,
  manageDisabled = false,
  bulkToggleLabel,
  bulkEnabled,
  bulkToggleDisabled = false,
  onBulkEnabledChange,
  bulkRemoveLabel,
  bulkRemoveTitle,
  bulkRemoveDisabled = false,
  onBulkRemove,
  removeDisabled = false,
  emptyTitle = "还没有添加资产",
  emptyDescription,
  emptyAction,
  columns = 2,
}: {
  domain: AssetRef["domain"];
  title: string;
  description?: string;
  manageLabel: string;
  rows: ConsumptionView[];
  external: ConsumptionView[];
  externalMode?: "summary" | "cards";
  externalTitle?: string;
  externalDescription?: string;
  present(asset: AssetRef): ConsumptionAssetPresentation;
  onManage(): void;
  manageIcon?: ReactNode;
  onOpenAsset?(asset: AssetRef): void;
  onEnabledChange?(item: ConsumptionView, enabled: boolean): void;
  enabledChangeDisabled?: boolean | ((item: ConsumptionView) => boolean);
  toggleKind?: "enabled" | "current";
  renderAction?(item: ConsumptionView): ReactNode;
  onRemove?(asset: AssetRef): void;
  onConverge?(item: ConsumptionView, action: ConvergenceAction): void;
  convergenceDisabled?: boolean;
  removeLabel?(name: string): string;
  manageDisabled?: boolean;
  bulkToggleLabel?: string;
  bulkEnabled?: boolean;
  bulkToggleDisabled?: boolean;
  onBulkEnabledChange?(enabled: boolean): void;
  bulkRemoveLabel?: string;
  bulkRemoveTitle?: string;
  bulkRemoveDisabled?: boolean;
  onBulkRemove?(): void;
  removeDisabled?: boolean;
  emptyTitle?: string;
  emptyDescription?: string;
  emptyAction?: ReactNode;
  columns?: 2 | 3;
}) {
  const domainRows = rows.filter((item) => item.asset.domain === domain);
  const domainExternal = external.filter((item) => item.asset.domain === domain);
  const items = [
    ...domainRows.map((item) => ({ item, external: false })),
    ...(externalMode === "cards" ? domainExternal.map((item) => ({ item, external: true })) : []),
  ];

  return (
    <section className="mux-agent-section mux-agent-resource-content mux-consumption-panel">
      <div className="mux-agent-section-head">
        <div>
          <h3>{title}</h3>
          {description && <p>{description}</p>}
        </div>
        <div className="mux-agent-section-actions">
          {bulkToggleLabel && bulkEnabled !== undefined && onBulkEnabledChange && (
            <div className="mux-agent-bulk-toggle">
              <span>{bulkToggleLabel}</span>
              <Switch
                checked={bulkEnabled}
                compact
                disabled={bulkToggleDisabled}
                ariaLabel={bulkEnabled ? `停用全部 ${title}` : `启用全部 ${title}`}
                title={bulkEnabled ? `停用全部 ${title}` : `启用全部 ${title}`}
                onChange={onBulkEnabledChange}
              />
            </div>
          )}
          {bulkRemoveLabel && onBulkRemove && (
            <button
              type="button"
              className="btn-danger"
              disabled={bulkRemoveDisabled}
              title={bulkRemoveTitle}
              onClick={onBulkRemove}
            >
              <TrashIcon className="w-3.5 h-3.5" />
              {bulkRemoveLabel}
            </button>
          )}
          <button
            type="button"
            className="btn-primary"
            disabled={manageDisabled}
            onClick={onManage}
          >
            {manageIcon}
            {manageLabel}
          </button>
        </div>
      </div>

      {externalMode === "summary" && domainExternal.length > 0 && (
        <div className="mux-consumption-external" role="status">
          <div>
            <strong>{externalTitle} {domainExternal.length}</strong>
            <span>{externalDescription}</span>
          </div>
          <ul>
            {domainExternal.slice(0, 3).map((item) => {
              const shared = item.asset.domain === "skill" && item.affected_agent_ids.length > 1;
              return (
                <li key={`${item.agent_id}:${item.asset.domain}:${assetIdentity(item.asset)}`}>
                  {present(item.asset).name}
                  {shared && <small>外部 · 共用 {item.affected_agent_ids.length}</small>}
                </li>
              );
            })}
          </ul>
        </div>
      )}

      {items.length === 0 ? (
        <div className="mux-consumption-empty">
          <PackageIcon className="w-7 h-7" />
          <strong>{emptyTitle}</strong>
          {emptyDescription && <span>{emptyDescription}</span>}
          {emptyAction}
        </div>
      ) : (
        <ul className="mux-consumption-list" data-columns={columns}>
          {items.map(({ item, external: isExternal }) => {
            const presentation = present(item.asset);
            const presentationDescription = presentation.description?.trim();
            const enabled = typeof item.enabled === "boolean" ? item.enabled : null;
            const toggleDisabled = typeof enabledChangeDisabled === "function"
              ? enabledChangeDisabled(item)
              : enabledChangeDisabled;
            const toggleLabel = toggleKind === "current"
              ? enabled
                ? `${presentation.name} 当前正在使用；请选择其他 Model 切换`
                : `使用 ${presentation.name}`
              : enabled
                ? `停用 ${presentation.name}`
                : `启用 ${presentation.name}`;
            return (
              <li
                key={`${item.agent_id}:${item.asset.domain}:${assetIdentity(item.asset)}`}
                data-status={item.status}
                data-enabled={isExternal || enabled === false ? "false" : undefined}
              >
                <span className="mux-consumption-icon">{presentation.icon}</span>
                <span className="mux-consumption-copy">
                  <span className="mux-consumption-title">
                    <strong>{presentation.name}</strong>
                    {presentation.meta && (
                      <span className="mux-consumption-meta">{presentation.meta}</span>
                    )}
                  </span>
                  {presentationDescription && <small>{presentationDescription}</small>}
                </span>
                {(item.status !== "synced" && !(isExternal && item.status === "external-added")
                  || item.available_actions.length > 0
                  || !isExternal && (renderAction || onEnabledChange && enabled !== null || onOpenAsset || onRemove)) && (
                  <span className="mux-consumption-controls">
                    {item.status !== "synced"
                      && !(isExternal && item.status === "external-added") && (
                      <ConsumptionStatus status={item.status} reason={item.reason} compact />
                    )}
                    <span className="mux-consumption-actions">
                      {onConverge && item.available_actions.length > 0 && (
                        <ConvergenceActions
                          actions={item.available_actions}
                          disabled={convergenceDisabled}
                          onAction={(action) => onConverge(item, action)}
                        />
                      )}
                      {renderAction?.(item)}
                      {!isExternal && onEnabledChange && enabled !== null && (
                        <Switch
                          checked={enabled}
                          compact
                          disabled={toggleDisabled}
                          ariaLabel={toggleLabel}
                          title={toggleLabel}
                          onChange={(next) => onEnabledChange(item, next)}
                        />
                      )}
                      {!isExternal && onOpenAsset && (
                        <button
                          type="button"
                          className="mux-consumption-open"
                          aria-label={`查看 ${presentation.name}`}
                          onClick={() => onOpenAsset(item.asset)}
                        >
                          <LinkIcon className="w-4 h-4" />
                        </button>
                      )}
                      {!isExternal && onRemove && (
                        <button
                          type="button"
                          className="mux-consumption-open mux-consumption-remove"
                          aria-label={removeLabel?.(presentation.name) ?? `从 Agent 移除 ${presentation.name}`}
                          disabled={removeDisabled}
                          onClick={() => onRemove(item.asset)}
                        >
                          <TrashIcon className="w-4 h-4" />
                        </button>
                      )}
                    </span>
                  </span>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
