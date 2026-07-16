import type {
  InventoryState,
  RiskLevel,
  SkillInventoryItem,
  SkillSource,
} from "../lib/types";
import { AgentStack } from "./ResourceWorkspace";
import { Avatar, Badge } from "./ui";

const stateLabels: Record<InventoryState, string> = {
  managed: "已托管",
  assigned: "已分配",
  external: "外部副本",
  locally_modified: "本地已修改",
  broken_link: "链接损坏",
  conflicting_link: "链接冲突",
  missing: "正文缺失",
  update_available: "有更新",
};

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
  return `导入副本 · ${source.original_path}`;
}

export function skillRiskLabel(level: RiskLevel | null) {
  if (level === "high") return "高风险";
  if (level === "medium") return "中风险";
  if (level === "low") return "未发现高风险模式";
  return "未审阅";
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
    <article
      className="mux-tile mux-skill-card"
      data-selected={selected ? "true" : undefined}
      data-risk={item.risk?.level ?? "unreviewed"}
      role="button"
      tabIndex={0}
      aria-label={`打开 Skill ${item.name} 详情`}
      aria-pressed={selected}
      onClick={onOpen}
      onKeyDown={(event) => {
        if (event.target !== event.currentTarget) return;
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onOpen();
        }
      }}
    >
      <header className="mux-skill-card-head">
        <Avatar seed={item.name} label="S" size={36} />
        <div className="mux-skill-card-identity">
          <h2 title={item.name}>{item.name}</h2>
          <p>{item.description}</p>
        </div>
      </header>

      <div className="mux-skill-card-provenance">
        <span title={skillSourceText(item.source)}>{skillSourceText(item.source)}</span>
        {item.source?.kind === "imported" && <Badge tone="info">Imported</Badge>}
        {item.resolved_revision ? (
          <code title={item.resolved_revision}>revision {item.resolved_revision.slice(0, 12)}</code>
        ) : (
          <span>未记录 revision</span>
        )}
      </div>

      <div className="mux-skill-card-status">
        <SkillRiskBadge level={item.risk?.level ?? null} />
        {item.states.map((state) => (
          <Badge
            key={state}
            tone={state === "managed" || state === "assigned" ? "success" : state === "external" ? "neutral" : "warning"}
          >
            {stateLabels[state]}
          </Badge>
        ))}
      </div>

      {item.update.error && (
        <p className="mux-skill-card-update-error">
          更新检查失败：{item.update.error}
          {item.update.retry_at ? ` · 可重试：${item.update.retry_at}` : ""}
        </p>
      )}

      <footer className="mux-skill-card-agents">
        <AgentStack ids={item.affected_agent_ids} />
      </footer>
    </article>
  );
}
