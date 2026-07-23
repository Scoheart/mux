import type {
  RiskLevel,
  SkillInventoryItem,
  SkillSource,
} from "../lib/types";
import { ResourceCard, ResourceKindIcon } from "./ResourceCard";
import { Badge } from "./ui";
import { PackageIcon } from "./icons";

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

export function SkillCard({
  item,
  selected,
  onOpen,
}: {
  item: SkillInventoryItem;
  selected: boolean;
  onOpen: () => void;
}) {
  return (
    <ResourceCard
      className="mux-skill-card"
      selected={selected}
      ariaLabel={`打开 Skill ${item.name} 详情`}
      onOpen={onOpen}
      identity={
        <>
          <ResourceKindIcon kind="skill">
            <PackageIcon className="w-4 h-4" />
          </ResourceKindIcon>
          <div className="mux-resource-card-copy">
            <div className="mux-resource-card-heading">
              <h2 title={item.name}>{item.name}</h2>
              {item.update.available && <Badge tone="info">有更新</Badge>}
            </div>
            <span className="mux-resource-card-code" title={skillSourceText(item.source)}>
              {skillSourceText(item.source)}
            </span>
          </div>
        </>
      }
    />
  );
}
