import { useEffect, useState } from "react";
import type {
  InventoryState,
  PlanRepairRequest,
  SkillCommandError,
  SkillDetail,
  SkillInventoryItem,
  SkillRiskFinding,
} from "../lib/types";
import {
  InspectorField,
  InspectorSection,
  ResourceInspector,
} from "./ResourceWorkspace";
import { SkillRiskBadge, skillSourceText } from "./SkillCard";
import { AgentGlyph, agentName } from "./brandIcons";
import { Avatar, Badge } from "./ui";
import {
  CalendarIcon,
  FolderIcon,
  LayersIcon,
  LinkIcon,
  RefreshIcon,
  TerminalIcon,
  TrashIcon,
} from "./icons";

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

export type SkillLifecycleIntent =
  | {
      kind: "import";
      identity: string;
      replaceConflicts: boolean;
    }
  | { kind: "update"; skillName: string; replaceLocalChanges: boolean }
  | { kind: "remove"; skillName: string }
  | {
      kind: "repair";
      skillName: string;
      repair: PlanRepairRequest["repair"];
    };

function sourceKindLabel(item: SkillInventoryItem) {
  if (!item.source) return "外部副本";
  if (item.source.kind === "github") return "GitHub";
  if (item.source.kind === "local") return "本地";
  if (item.source.kind === "archive") return "压缩包";
  return "Imported";
}

function groupFindings(findings: SkillRiskFinding[]) {
  const groups = new Map<string, { path: string; level: SkillRiskFinding["level"]; reasons: Set<string> }>();
  const rank = { low: 0, medium: 1, high: 2 };
  for (const finding of findings) {
    const group = groups.get(finding.path) ?? { path: finding.path, level: finding.level, reasons: new Set<string>() };
    if (rank[finding.level] > rank[group.level]) group.level = finding.level;
    group.reasons.add(finding.reason);
    groups.set(finding.path, group);
  }
  return [...groups.values()].sort((a, b) => rank[b.level] - rank[a.level] || a.path.localeCompare(b.path));
}

function shortDate(value: string | null) {
  if (!value) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleDateString();
}

export function SkillInspector({
  item,
  detail,
  loading,
  error,
  onClose,
  onPlan,
  planning = false,
  readOnly = false,
}: {
  item: SkillInventoryItem;
  detail: SkillDetail | null;
  loading: boolean;
  error: SkillCommandError | null;
  onClose: () => void;
  onPlan?: (intent: SkillLifecycleIntent) => void;
  planning?: boolean;
  readOnly?: boolean;
}) {
  const [replaceConflicts, setReplaceConflicts] = useState(false);
  const [replaceLocalChanges, setReplaceLocalChanges] = useState(false);
  useEffect(() => {
    setReplaceConflicts(false);
    setReplaceLocalChanges(false);
  }, [item.identity]);
  const managedRecord = item.source !== null;
  const centralManaged = managedRecord && item.location.kind === "central";
  const healthyManaged = centralManaged && item.states.includes("managed");
  const updateEligible =
    centralManaged &&
    item.update.available &&
    (healthyManaged || item.states.includes("locally_modified"));
  const external = item.states.includes("external");
  const repair = !managedRecord
    ? null
    : item.location.kind === "central" &&
        (item.states.includes("missing") || item.states.includes("broken_link"))
      ? ({ kind: "central" } as const)
      : item.location.kind === "agent_target" &&
          (item.states.includes("missing") || item.states.includes("broken_link"))
        ? ({ kind: "target", target_id: item.location.target_id } as const)
        : null;
  const disabled = planning || readOnly;
  const findings = groupFindings(item.risk?.findings ?? []);
  const footer = onPlan ? (
    <div className="mux-skill-inspector-actions">
      {external && (
        <label className="mux-skill-replacement-choice">
          <input
            type="checkbox"
            checked={replaceConflicts}
            disabled={disabled}
            onChange={(event) => setReplaceConflicts(event.target.checked)}
          />
          <span>备份并替换同名中央副本</span>
        </label>
      )}
      {centralManaged && item.states.includes("locally_modified") && item.update.available && (
        <label className="mux-skill-replacement-choice">
          <input
            type="checkbox"
            checked={replaceLocalChanges}
            disabled={disabled}
            onChange={(event) => setReplaceLocalChanges(event.target.checked)}
          />
          <span>保留备份并替换本地更改</span>
        </label>
      )}
      {external && (
        <button
          type="button"
          className="btn-primary"
          disabled={disabled}
          onClick={() =>
            onPlan({
              kind: "import",
              identity: item.identity,
              replaceConflicts,
            })
          }
        >
          导入
        </button>
      )}
      {updateEligible && (
        <button
          type="button"
          className="btn-primary"
          disabled={disabled}
          onClick={() =>
            onPlan({
              kind: "update",
              skillName: item.name,
              replaceLocalChanges,
            })
          }
        >
          更新
        </button>
      )}
      {repair && (
        <button
          type="button"
          className="btn-secondary"
          disabled={disabled}
          onClick={() =>
            onPlan({ kind: "repair", skillName: item.name, repair })
          }
        >
          修复
        </button>
      )}
      {centralManaged && (
        <button
          type="button"
          className="btn-danger"
          disabled={disabled}
          onClick={() => onPlan({ kind: "remove", skillName: item.name })}
        >
          <TrashIcon className="w-4 h-4" />
          移除
        </button>
      )}
      {planning && <span role="status">正在生成操作计划…</span>}
    </div>
  ) : undefined;

  return (
    <ResourceInspector
      title={item.name}
      avatar={<Avatar seed={item.name} kind="skill" size={40} />}
      subtitle={
        <div className="mux-skill-inspector-badges">
          <Badge tone={item.source?.kind === "github" ? "info" : "neutral"}>
            {sourceKindLabel(item)}
          </Badge>
        </div>
      }
      onClose={onClose}
      footer={footer}
    >
      <div className="mux-skill-detail-summary" key={item.identity}>
        <p className="mux-skill-inspector-description">{item.description || "暂无说明"}</p>
        <div className="mux-skill-detail-source">
          <LinkIcon className="w-3.5 h-3.5" />
          <span>{item.source?.kind === "github" ? `${item.source.owner}/${item.source.repo}` : skillSourceText(item.source)}</span>
          {item.updated_at && <small>更新于 {shortDate(item.updated_at)}</small>}
        </div>
        <div className="mux-skill-inspector-state-list">
          <SkillRiskBadge level={item.risk?.level ?? null} />
          {item.states.filter((state) => state !== "assigned").map((state) => (
            <Badge key={state} tone={state === "managed" ? "success" : state === "external" ? "neutral" : "warning"}>
              {stateLabels[state]}
            </Badge>
          ))}
        </div>
        {item.update.error && <p className="mux-skill-inspector-update-error">更新检查失败：{item.update.error}</p>}

        {item.affected_agent_ids.length > 0 && (
          <InspectorSection title={`关联 Agent · ${item.affected_agent_ids.length}`} icon={<LayersIcon className="w-4 h-4" />}>
            <div className="mux-skill-detail-agent-chips">
              {item.affected_agent_ids.slice(0, 5).map((id) => (
                <span key={id}><AgentGlyph id={id} size={18} />{agentName(id)}</span>
              ))}
            </div>
            {item.affected_agent_ids.length > 5 && (
              <details className="mux-skill-detail-more-agents">
                <summary>另外 {item.affected_agent_ids.length - 5} 个</summary>
                <div className="mux-skill-detail-agent-chips">
                  {item.affected_agent_ids.slice(5).map((id) => (
                    <span key={id}><AgentGlyph id={id} size={18} />{agentName(id)}</span>
                  ))}
                </div>
              </details>
            )}
          </InspectorSection>
        )}

        <div className="mux-skill-detail-disclosures">
          <details className="mux-skill-detail-disclosure">
            <summary><LayersIcon className="w-4 h-4" /><strong>风险详情</strong>
              <span>{item.risk ? `${item.risk.finding_count} 条 · ${findings.length} 个文件` : "尚未检查"}</span>
            </summary>
            <div className="mux-skill-inspector-findings">
              {!item.risk ? <p>尚未完成风险检查。</p> : findings.length === 0 ? <p>未发现风险提示。</p> : (
                <ul>{findings.map((finding) => (
                  <li key={finding.path}>
                    <div className="mux-skill-inspector-finding-head">
                      <code>{finding.path}</code><SkillRiskBadge level={finding.level} />
                    </div>
                    <p>{[...finding.reasons].join("；")}</p>
                  </li>
                ))}</ul>
              )}
              {item.risk?.findings_truncated && <p className="mux-skill-inspector-truncation">
                当前包含 {item.risk.findings.length} / {item.risk.finding_count} 条检查结果
              </p>}
            </div>
          </details>

          <details className="mux-skill-detail-disclosure">
            <summary><TerminalIcon className="w-4 h-4" /><strong>查看正文</strong><span>SKILL.md</span></summary>
            {loading ? <p className="mux-skill-inspector-loading" role="status">正在读取…</p>
              : error ? <p className="mux-skill-inspector-error" role="alert">读取失败：{error.message}</p>
              : detail ? <>
                {detail.skill_md_truncated && <p className="mux-skill-inspector-truncation">正文预览已截断</p>}
                <pre className="mux-skill-preview" aria-label="SKILL.md 纯文本预览">{detail.skill_md}</pre>
              </> : <p className="mux-skill-inspector-empty">尚未加载正文。</p>}
          </details>

          <details className="mux-skill-detail-disclosure">
            <summary><FolderIcon className="w-4 h-4" /><strong>文件</strong><span>{detail ? `${detail.files.length} 个` : loading ? "读取中…" : "未加载"}</span></summary>
            {detail && <ul className="mux-skill-file-tree mux-skill-detail-files" aria-label="Skill 文件树">
              {detail.files.map((file) => (
                <li key={file.path}>
                  <code title={file.path}>{file.path}</code>
                  <span>{file.kind === "symlink" ? `链接 → ${file.link_target ?? "未知目标"}`
                    : file.size < 1024 ? `${file.size} B` : `${Math.round(file.size / 1024)} KB`}</span>
                </li>
              ))}
            </ul>}
            {error && <p className="mux-skill-inspector-error" role="alert">读取失败：{error.message}</p>}
          </details>

          <details className="mux-skill-detail-disclosure">
            <summary><RefreshIcon className="w-4 h-4" /><strong>技术信息</strong><span>版本与校验值</span></summary>
            <InspectorField icon={<LinkIcon className="w-4 h-4" />} label="完整来源" mono>{skillSourceText(item.source)}</InspectorField>
            <InspectorField icon={<RefreshIcon className="w-4 h-4" />} label="Revision" mono>{item.resolved_revision ?? "未记录"}</InspectorField>
            <InspectorField icon={<TerminalIcon className="w-4 h-4" />} label="内容哈希" mono>{item.content_hash ?? "未记录"}</InspectorField>
            <InspectorField icon={<CalendarIcon className="w-4 h-4" />} label="安装时间" mono>{item.installed_at ?? "未记录"}</InspectorField>
            <InspectorField icon={<CalendarIcon className="w-4 h-4" />} label="更新时间" mono>{item.updated_at ?? "未记录"}</InspectorField>
            {item.risk && item.risk.findings.length > 0 && (
              <details className="mux-skill-detail-raw"><summary>原始检查记录</summary>
                <ul>{item.risk.findings.map((finding, index) => (
                  <li key={`${finding.rule_id}:${finding.path}:${index}`}>
                    <code>{finding.path}{finding.line === null ? "" : `:${finding.line}`} · {finding.rule_id} · v{finding.rule_version}</code>
                    <p>{finding.reason}</p>
                  </li>
                ))}</ul>
              </details>
            )}
            {detail && <details className="mux-skill-detail-raw"><summary>文件校验值</summary>
              <ul>{detail.files.map((file) => <li key={file.path}><code>{file.path}</code><code>{file.sha256}</code></li>)}</ul>
            </details>}
          </details>
        </div>
      </div>
    </ResourceInspector>
  );
}
