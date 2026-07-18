import type { ReactNode } from "react";
import type { AssetRef, ConsumptionView } from "../lib/types";
import { assetIdentity } from "../lib/consumption";
import { LinkIcon, PackageIcon, PlusIcon } from "./icons";
import { ConsumptionStatus } from "./ConsumptionStatus";

export interface ConsumptionAssetPresentation {
  name: string;
  description?: string;
  icon?: ReactNode;
  meta?: ReactNode;
}

export function AgentConsumptionPanel({
  title,
  description,
  manageLabel,
  rows,
  external,
  present,
  onManage,
  onOpenAsset,
  manageDisabled = false,
  emptyAction,
}: {
  title: string;
  description: string;
  manageLabel: string;
  rows: ConsumptionView[];
  external: ConsumptionView[];
  present(asset: AssetRef): ConsumptionAssetPresentation;
  onManage(): void;
  onOpenAsset?(asset: AssetRef): void;
  manageDisabled?: boolean;
  emptyAction?: ReactNode;
}) {
  return (
    <section className="mux-agent-section mux-agent-resource-content mux-consumption-panel">
      <div className="mux-agent-section-head">
        <div>
          <h3>{title}</h3>
          <p>{description}</p>
        </div>
        <button
          type="button"
          className="btn-primary"
          disabled={manageDisabled}
          onClick={onManage}
        >
          <PlusIcon className="w-3.5 h-3.5" />
          {manageLabel}
        </button>
      </div>

      {external.length > 0 && (
        <div className="mux-consumption-external" role="status">
          <div>
            <strong>检测到 {external.length} 个外部配置</strong>
            <span>它们是只读 observed state；导入中央资产后才能建立消费关系。</span>
          </div>
          <ul>
            {external.slice(0, 3).map((item) => (
              <li key={`${item.agent_id}:${item.asset.domain}:${assetIdentity(item.asset)}`}>
                {present(item.asset).name}
              </li>
            ))}
          </ul>
        </div>
      )}

      {rows.length === 0 ? (
        <div className="mux-consumption-empty">
          <PackageIcon className="w-7 h-7" />
          <strong>尚未使用中央资产</strong>
          <span>从中央资产库选择后，MUX 会生成影响计划再同步 Agent。</span>
          {emptyAction}
        </div>
      ) : (
        <ul className="mux-consumption-list">
          {rows.map((item) => {
            const presentation = present(item.asset);
            return (
              <li
                key={`${item.agent_id}:${item.asset.domain}:${assetIdentity(item.asset)}`}
                data-status={item.status}
              >
                <span className="mux-consumption-icon">{presentation.icon}</span>
                <span className="mux-consumption-copy">
                  <strong>{presentation.name}</strong>
                  <small>{presentation.description ?? assetIdentity(item.asset)}</small>
                  {item.reason && item.status !== "synced" && <code>{item.reason}</code>}
                </span>
                {presentation.meta && (
                  <span className="mux-consumption-meta">{presentation.meta}</span>
                )}
                <ConsumptionStatus status={item.status} reason={item.reason} />
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
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
