import { useState } from "react";
import type { AssetOperationPlan, AssetRef } from "../lib/types";
import { assetIdentity } from "../lib/consumption";
import { DialogShell } from "./DialogShell";

function assetLabel(asset: AssetRef) {
  const domain = asset.domain === "mcp" ? "MCP" : asset.domain === "model" ? "Model" : "Skill";
  return `${domain} · ${assetIdentity(asset)}`;
}

export function AssetOperationReviewDialog({
  plan,
  busy,
  error,
  onCommit,
  onCancel,
}: {
  plan: AssetOperationPlan;
  busy: boolean;
  error?: string | null;
  onCommit(conflictConfirmation?: string): Promise<unknown> | unknown;
  onCancel(): Promise<unknown> | unknown;
}) {
  const [replaceDrift, setReplaceDrift] = useState(false);
  const title = plan.kind === "update-asset"
    ? "审阅中央资产变更"
    : plan.kind === "delete-asset"
      ? "审阅中央资产删除"
      : "审阅资产消费变更";
  const commitLabel = plan.kind === "delete-asset" ? "确认删除并同步" : "确认并同步";
  return (
    <DialogShell
      kind="review"
      size="md"
      title={title}
      subtitle={`将影响 ${plan.affected_agent_ids.length} 个 Agent、${plan.target_files.length} 个目标。`}
      busy={busy}
      onClose={() => void onCancel()}
      status={!plan.can_commit
        ? <span className="mux-review-error">存在未解决的漂移或冲突，当前计划不可提交。</span>
        : plan.requires_conflict_confirmation
          ? <span className="mux-review-error">继续会覆盖审阅中列出的漂移字段，并在写入前保留备份。</span>
          : null}
      footerStart={error ? <span className="mux-review-error">{error}</span> : null}
      footerEnd={
        <>
          <button type="button" className="btn-ghost" disabled={busy} onClick={() => void onCancel()}>取消计划</button>
          <button
            type="button"
            className="btn-primary"
            disabled={busy || !plan.can_commit || (plan.requires_conflict_confirmation && !replaceDrift)}
            onClick={() => void onCommit(plan.requires_conflict_confirmation ? plan.candidate_hash : undefined)}
          >
            {busy ? "同步中…" : commitLabel}
          </button>
        </>
      }
    >
      <div className="mux-review-content mux-asset-review">
        {plan.central_changes.length > 0 && (
          <section>
            <h3>中央资产变化</h3>
            <ul>
              {plan.central_changes.map((change) => (
                <li key={`${change.action}:${assetIdentity(change.asset)}`}>
                  <span data-action={change.action}>
                    {change.action === "create" ? "创建" : change.action === "update" ? "更新" : "删除"}
                  </span>
                  <code>{assetLabel(change.asset)}</code>
                  {change.summary.length > 0 && <small>{change.summary.join("；")}</small>}
                </li>
              ))}
            </ul>
          </section>
        )}
        <section>
          <h3>关系变化</h3>
          {plan.relationship_changes.length === 0 ? <p>desired relationship 无变化。</p> : (
            <ul>
              {plan.relationship_changes.map((change, index) => (
                <li key={`${change.agent_id}:${assetIdentity(change.asset)}:${index}`}>
                  <span data-action={change.action}>{change.action === "add" ? "使用" : "解除"}</span>
                  <strong>{change.agent_id}</strong>
                  <code>{assetLabel(change.asset)}</code>
                </li>
              ))}
            </ul>
          )}
        </section>
        <section>
          <h3>写入目标</h3>
          <ul>{plan.target_files.map((path) => <li key={path}><code>{path}</code></li>)}</ul>
        </section>
        {plan.warnings.length > 0 && (
          <section className="mux-asset-review-warnings">
            <h3>需要处理</h3>
            <ul>{plan.warnings.map((warning) => <li key={warning}>{warning}</li>)}</ul>
          </section>
        )}
        {plan.requires_conflict_confirmation && (
          <label className="mux-model-check mux-asset-conflict-confirmation">
            <input
              type="checkbox"
              checked={replaceDrift}
              disabled={busy}
              onChange={(event) => setReplaceDrift(event.target.checked)}
            />
            我已审阅上述漂移，允许用中央资产覆盖这些 owned fields
          </label>
        )}
      </div>
    </DialogShell>
  );
}
