import type {
  RiskLevel,
  SkillInventoryItem,
  SkillSource,
} from "../lib/types";
import { useTranslation } from "react-i18next";
import { ResourceKindIcon } from "./ResourceCard";
import { Badge } from "./ui";

function appendSubpath(base: string, subpath: string) {
  return subpath ? `${base} / ${subpath}` : base;
}

export function skillSourceText(source: SkillSource | null) {
  if (!source) return "外部副本 · 来源未知";
  if (source.kind === "github") {
    return appendSubpath(`GitHub · ${source.owner}/${source.repo}`, source.subpath);
  }
  if (source.kind === "local") {
    return appendSubpath(`本地 · ${source.path}`, source.subpath);
  }
  if (source.kind === "archive") {
    return appendSubpath(`压缩包 · ${source.path}`, source.subpath);
  }
  return `导入副本 · ${source.original_path}`;
}

export function skillRiskLabel(level: RiskLevel | null) {
  if (level === "high") return "高风险";
  if (level === "medium") return "中风险";
  if (level === "low") return "未发现高风险模式";
  return "尚未检查";
}

export function SkillRiskBadge({
  level,
  label,
}: {
  level: RiskLevel | null;
  label?: string;
}) {
  return (
    <span
      className="mux-skill-risk-badge"
      data-level={level ?? "unreviewed"}
    >
      {label ?? skillRiskLabel(level)}
    </span>
  );
}

const attentionStates = new Set([
  "locally_modified",
  "broken_link",
  "conflicting_link",
  "missing",
]);

function skillAssetState(item: SkillInventoryItem) {
  if (
    item.update.available ||
    item.risk?.level === "high" ||
    item.states.some((state) => attentionStates.has(state))
  ) {
    return { labelKey: "needsAttention" as const, tone: "warning" as const };
  }
  if (item.states.includes("external")) return { labelKey: "external" as const, tone: "info" as const };
  return { labelKey: "normal" as const, tone: "success" as const };
}

function skillUpdateState(item: SkillInventoryItem) {
  if (item.update.available) return { labelKey: "updateAvailable" as const, tone: "info" as const };
  if (item.update.error) return { labelKey: "updateFailed" as const, tone: "warning" as const };
  if (item.update.checked_at) return { labelKey: "upToDate" as const, tone: "success" as const };
  return { labelKey: "notChecked" as const, tone: "neutral" as const };
}

export function SkillCard({
  item,
  selected,
  onOpen,
}: {
  item: SkillInventoryItem;
  selected: boolean;
  onOpen: () => void;
}) {
  const { t } = useTranslation();
  const source = skillSourceText(item.source);
  const assetState = skillAssetState(item);
  const updateState = skillUpdateState(item);

  return (
    <button
      type="button"
      className="mux-asset-list-row mux-skill-list-row"
      data-selected={selected ? "true" : undefined}
      data-attention={assetState.labelKey === "needsAttention" ? "warning" : undefined}
      aria-label={t("centralAssets.openSkillDetails", { name: item.name })}
      aria-pressed={selected}
      onClick={onOpen}
    >
      <span className="mux-asset-list-identity">
        <ResourceKindIcon kind="skill" seed={item.name} />
        <span className="mux-asset-list-copy">
          <h2 title={item.name}>{item.name}</h2>
          <span className="mux-skill-list-description" title={item.description}>
            {item.description || t("centralAssets.noDescription")}
          </span>
        </span>
      </span>
      <span className="mux-asset-list-stack mux-asset-list-source" title={source}>
        <span>{source}</span>
        {item.resolved_revision && <code>rev {item.resolved_revision.slice(0, 10)}</code>}
      </span>
      <span className="mux-skill-list-risk">
        <SkillRiskBadge
          level={item.risk?.level ?? null}
          label={item.risk?.level === "low" ? t("centralAssets.lowRisk") : undefined}
        />
      </span>
      <span className="mux-skill-list-update">
        <Badge tone={updateState.tone}>
          {t(`centralAssets.${updateState.labelKey}`)}
        </Badge>
      </span>
      <span className="mux-asset-list-status">
        <Badge tone={assetState.tone}>
          {t(`centralAssets.${assetState.labelKey}`)}
        </Badge>
      </span>
    </button>
  );
}
