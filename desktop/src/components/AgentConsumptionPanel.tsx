import type { ReactNode } from "react";
import type { AssetRef, ConsumptionView } from "../lib/types";
import { assetIdentity } from "../lib/consumption";
import { LinkIcon, PackageIcon, PlusIcon, XIcon } from "./icons";
import { ConsumptionStatus } from "./ConsumptionStatus";
import { Switch } from "./ui";

export interface ConsumptionAssetPresentation {
  name: string;
  description?: string;
  icon?: ReactNode;
  meta?: ReactNode;
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
  externalAction,
  present,
  onManage,
  manageIcon = <PlusIcon className="w-3.5 h-3.5" />,
  onOpenAsset,
  onEnabledChange,
  enabledChangeDisabled,
  renderAction,
  renderExternalAction,
  onRemove,
  removeLabel,
  manageDisabled = false,
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
  externalAction?: ReactNode;
  present(asset: AssetRef): ConsumptionAssetPresentation;
  onManage(): void;
  manageIcon?: ReactNode;
  onOpenAsset?(asset: AssetRef): void;
  onEnabledChange?(item: ConsumptionView, enabled: boolean): void;
  enabledChangeDisabled?: boolean | ((item: ConsumptionView) => boolean);
  renderAction?(item: ConsumptionView): ReactNode;
  renderExternalAction?(item: ConsumptionView): ReactNode;
  onRemove?(asset: AssetRef): void;
  removeLabel?(name: string): string;
  manageDisabled?: boolean;
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

      {externalMode === "summary" && domainExternal.length > 0 && (
        <div className="mux-consumption-external" role="status">
          <div>
            <strong>{externalTitle} {domainExternal.length}</strong>
            <span>{externalDescription}</span>
            {externalAction}
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
            const enabled = typeof item.enabled === "boolean" ? item.enabled : null;
            const toggleDisabled = typeof enabledChangeDisabled === "function"
              ? enabledChangeDisabled(item)
              : enabledChangeDisabled;
            const externalActionNode = isExternal ? renderExternalAction?.(item) : null;
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
                  <small>{presentation.description ?? assetIdentity(item.asset)}</small>
                </span>
                {(isExternal || item.status !== "synced") && (
                  <ConsumptionStatus status={item.status} reason={item.reason} />
                )}
                {isExternal ? externalActionNode && (
                  <span className="mux-consumption-actions">
                    {externalActionNode}
                  </span>
                ) : (renderAction || onEnabledChange && enabled !== null || onOpenAsset || onRemove) && (
                  <span className="mux-consumption-actions">
                    {renderAction?.(item)}
                    {onEnabledChange && enabled !== null && (
                      <Switch
                        checked={enabled}
                        compact
                        disabled={toggleDisabled}
                        ariaLabel={enabled ? `停用 ${presentation.name}` : `启用 ${presentation.name}`}
                        title={enabled ? `停用 ${presentation.name}` : `启用 ${presentation.name}`}
                        onChange={(next) => onEnabledChange(item, next)}
                      />
                    )}
                    {onOpenAsset && (
                      <button
                        type="button"
                        className="mux-consumption-open"
                        aria-label={`查看 ${presentation.name}`}
                        onClick={() => onOpenAsset(item.asset)}
                      >
                        <LinkIcon className="w-4 h-4" />
                      </button>
                    )}
                    {onRemove && (
                      <button
                        type="button"
                        className="mux-consumption-open mux-consumption-remove"
                        aria-label={removeLabel?.(presentation.name) ?? `从 Agent 移除 ${presentation.name}`}
                        disabled={removeDisabled}
                        onClick={() => onRemove(item.asset)}
                      >
                        <XIcon className="w-4 h-4" />
                      </button>
                    )}
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
