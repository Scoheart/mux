import { useEffect, useState } from "react";
import type {
  InventoryState,
  PlanRepairRequest,
  SkillCommandError,
  SkillDetail,
  SkillInventoryItem,
} from "../lib/types";
import {
  InspectorField,
  InspectorSection,
  ResourceInspector,
} from "./ResourceWorkspace";
import { SkillRiskBadge, skillSourceText } from "./SkillCard";
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
          移除
        </button>
      )}
      {planning && <span role="status">正在生成操作计划…</span>}
    </div>
  ) : undefined;

  return (
    <ResourceInspector
      title={item.name}
      avatar={<Avatar seed={item.name} label="S" size={40} />}
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
      <p className="mux-skill-inspector-description">{item.description}</p>

      <InspectorSection title="来源与版本">
        <InspectorField label="来源" mono>
          {skillSourceText(item.source)}
        </InspectorField>
        <InspectorField label="Revision" mono>
          {item.resolved_revision ?? "未记录"}
        </InspectorField>
        <InspectorField label="内容哈希" mono>
          {item.content_hash ?? "未记录"}
        </InspectorField>
        <InspectorField label="安装时间" mono>
          {item.installed_at ?? "未记录"}
        </InspectorField>
        <InspectorField label="更新时间" mono>
          {item.updated_at ?? "未记录"}
        </InspectorField>
      </InspectorSection>

      <InspectorSection title="状态与风险">
        <div className="mux-skill-inspector-state-list">
          <SkillRiskBadge level={item.risk?.level ?? null} />
          {item.states.filter((state) => state !== "assigned").map((state) => (
            <Badge
              key={state}
              tone={state === "managed" ? "success" : state === "external" ? "neutral" : "warning"}
            >
              {stateLabels[state]}
            </Badge>
          ))}
        </div>

        {item.update.error && (
          <p className="mux-skill-inspector-update-error">
            更新检查失败：{item.update.error}
            {item.update.retry_at ? ` · 可重试：${item.update.retry_at}` : ""}
          </p>
        )}

        {item.risk ? (
          <div className="mux-skill-inspector-findings">
            {item.risk.findings.length === 0 ? (
              <p>未发现需要展示的风险证据。</p>
            ) : (
              <ul>
                {item.risk.findings.map((finding, index) => (
                  <li key={`${finding.rule_id}:${finding.path}:${finding.line ?? "file"}:${index}`}>
                    <div className="mux-skill-inspector-finding-head">
                      <code>
                        {finding.path}
                        {finding.line === null ? "" : `:${finding.line}`}
                      </code>
                      <SkillRiskBadge
                        level={finding.level}
                        label={finding.level === "low" ? "提示" : undefined}
                      />
                    </div>
                    <p>{finding.reason}</p>
                    <code>{finding.rule_id} · v{finding.rule_version}</code>
                  </li>
                ))}
              </ul>
            )}
            {item.risk.findings_truncated && (
              <p className="mux-skill-inspector-truncation">
                已显示 {item.risk.findings.length} / {item.risk.finding_count} 条证据
              </p>
            )}
          </div>
        ) : (
          <p className="mux-skill-inspector-unreviewed">尚未执行风险审阅。</p>
        )}
      </InspectorSection>

      {loading ? (
        <InspectorSection title="内容">
          <p className="mux-skill-inspector-loading" role="status">正在读取 Skill 详情…</p>
        </InspectorSection>
      ) : error ? (
        <InspectorSection title="内容">
          <p className="mux-skill-inspector-error" role="alert">
            读取详情失败：{error.message}
            {error.retry_at ? ` · 可重试：${error.retry_at}` : ""}
          </p>
        </InspectorSection>
      ) : detail ? (
        <>
          <InspectorSection title="文件">
            <ul className="mux-skill-file-tree" aria-label="Skill 文件树">
              {detail.files.map((file) => (
                <li key={file.path}>
                  <code>{file.path}</code>
                  <span>
                    {file.kind === "symlink" ? `符号链接 → ${file.link_target ?? "未知目标"}` : `${file.size} bytes`}
                    {file.executable ? " · 可执行" : ""}
                  </span>
                  <code title={file.sha256}>{file.sha256}</code>
                </li>
              ))}
            </ul>
          </InspectorSection>

          <InspectorSection title="SKILL.md">
            {detail.skill_md_truncated && (
              <p className="mux-skill-inspector-truncation">SKILL.md 预览已截断</p>
            )}
            <pre className="mux-skill-preview" aria-label="SKILL.md 纯文本预览">
              {detail.skill_md}
            </pre>
          </InspectorSection>
        </>
      ) : (
        <InspectorSection title="内容">
          <p className="mux-skill-inspector-empty">尚未加载 Skill 详情。</p>
        </InspectorSection>
      )}
    </ResourceInspector>
  );
}
