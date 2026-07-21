import { useState } from "react";
import type { AssetOperationPlan, AssetRef } from "../lib/types";
import { assetIdentity } from "../lib/consumption";
import { DialogShell } from "./DialogShell";

function assetLabel(asset: AssetRef) {
  const domain = asset.domain === "mcp" ? "MCP" : asset.domain === "model" ? "Model" : "Skill";
  return `${domain} · ${assetIdentity(asset)}`;
}

function agentActionCopy(plan: AssetOperationPlan) {
  const domain = plan.domain_plan.domain;
  const asset = domain === "mcp" ? "MCP" : domain === "model" ? "Model" : "Skill";
  const hasAdd = plan.relationship_changes.some((change) => change.action === "add");
  const hasRemove = plan.relationship_changes.some((change) => change.action === "remove");
  if (domain === "model") {
    const states = plan.model_state_changes;
    if (states.some((change) => change.reason === "model_activated") && !hasAdd && !hasRemove) {
      return { title: "确认切换当前 Model", commit: "切换当前 Model", busy: "切换中…" };
    }
    if (hasAdd && !hasRemove) {
      return { title: "确认添加 Model", commit: "添加 Model", busy: "添加中…" };
    }
    if (states.some((change) => change.reason === "model_disabled")) {
      return { title: "确认停用 Model", commit: "停用 Model", busy: "停用中…" };
    }
    if (states.some((change) => change.reason === "model_enabled")) {
      return { title: "确认启用 Model", commit: "启用 Model", busy: "启用中…" };
    }
  }
  if (hasAdd && !hasRemove) {
    return { title: `确认添加 ${asset}`, commit: `添加 ${asset}`, busy: "添加中…" };
  }
  if (hasRemove && !hasAdd) {
    return { title: `确认移除 ${asset}`, commit: `移除 ${asset}`, busy: "移除中…" };
  }
  return { title: `确认更新 ${asset}`, commit: `更新 ${asset}`, busy: "更新中…" };
}

function modelStateLabel(state: { added: boolean; enabled: boolean; active: boolean }) {
  if (!state.added) return "未添加";
  if (!state.enabled) return "已添加 · 已停用";
  if (state.active) return "已启用 · 当前";
  return "已启用 · 非当前";
}

function configurationChanges(plan: AssetOperationPlan) {
  if (plan.domain_plan.domain !== "agent-configuration") return [];
  const { before, after } = plan.domain_plan;
  const rows: Array<{ label: string; before: string; after: string }> = [];
  if (before.mcp_path !== after.mcp_path) {
    rows.push({ label: "MCP", before: before.mcp_path, after: after.mcp_path });
  }
  const beforeModels = before.model_paths.join(" · ");
  const afterModels = after.model_paths.join(" · ");
  if (beforeModels !== afterModels) {
    rows.push({ label: "Model", before: beforeModels, after: afterModels });
  }
  if (before.skills_global_dir !== after.skills_global_dir) {
    rows.push({
      label: "Skills",
      before: before.skills_global_dir ?? "未接入",
      after: after.skills_global_dir ?? "未接入",
    });
  }
  return rows;
}

export function AssetOperationReviewDialog({
  plan,
  busy,
  error,
  agentId,
  agentName,
  onCommit,
  onCancel,
}: {
  plan: AssetOperationPlan;
  busy: boolean;
  error?: string | null;
  agentId?: string;
  agentName?: string;
  onCommit(conflictConfirmation?: string): Promise<unknown> | unknown;
  onCancel(): Promise<unknown> | unknown;
}) {
  const [replaceDrift, setReplaceDrift] = useState(false);
  const isConfiguration = plan.kind === "update-configuration";
  const isAgentSkillPlan = Boolean(
    agentId && agentName && plan.kind === "set-consumption" && plan.domain_plan.domain === "skill",
  );
  const compatibleAgentCount = isAgentSkillPlan
    ? plan.affected_agent_ids.filter((id) => id !== agentId).length
    : 0;
  const configChanges = configurationChanges(plan);
  const agentCopy = agentName && plan.kind === "set-consumption" ? agentActionCopy(plan) : null;
  const title = isConfiguration ? "确认修改配置" : agentCopy?.title ?? (plan.kind === "update-asset"
    ? "审阅中央资产变更"
    : plan.kind === "delete-asset"
      ? "审阅中央资产删除"
      : "审阅资产消费变更");
  const commitLabel = isConfiguration ? "保存配置" : agentCopy?.commit ?? (plan.kind === "delete-asset" ? "确认删除并同步" : "确认并同步");
  const subtitle = agentName
    ? compatibleAgentCount > 0
      ? `${agentName} · 同一目录也被 ${compatibleAgentCount} 个 Agent 读取`
      : plan.affected_agent_ids.length > 1
      ? `${agentName} · 另影响 ${plan.affected_agent_ids.length - 1} 个 Agent`
      : agentName
    : `${plan.affected_agent_ids.length} 个 Agent · ${plan.target_files.length} 个目标`;
  return (
    <DialogShell
      kind="review"
      size="md"
      title={title}
      subtitle={subtitle}
      busy={busy}
      onClose={() => void onCancel()}
      status={!plan.can_commit
        ? <span className="mux-review-error">存在冲突，暂不可继续。</span>
        : plan.requires_conflict_confirmation
          ? <span className="mux-review-error">将覆盖差异，写入前备份。</span>
          : null}
      footerStart={error ? <span className="mux-review-error">{error}</span> : null}
      footerEnd={
        <>
          <button type="button" className="btn-ghost" disabled={busy} onClick={() => void onCancel()}>取消</button>
          <button
            type="button"
            className="btn-primary"
            disabled={busy || !plan.can_commit || (plan.requires_conflict_confirmation && !replaceDrift)}
            onClick={() => void onCommit(plan.requires_conflict_confirmation ? plan.candidate_hash : undefined)}
          >
            {busy ? (isConfiguration ? "保存中…" : agentCopy?.busy ?? "同步中…") : commitLabel}
          </button>
        </>
      }
    >
      <div className="mux-review-content mux-asset-review">
        {isConfiguration && configChanges.length > 0 && (
          <section className="mux-config-review">
            <h3>配置位置</h3>
            <ul>
              {configChanges.map((change) => (
                <li key={change.label}>
                  <strong>{change.label}</strong>
                  <code>{change.before}</code>
                  <span>→</span>
                  <code>{change.after}</code>
                </li>
              ))}
            </ul>
          </section>
        )}
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
        {plan.model_state_changes.length > 0 && (
          <section>
            <h3>Model 状态变化</h3>
            <ul>
              {plan.model_state_changes.map((change) => (
                <li key={`${change.agent_id}:${change.profile_id}`}>
                  <strong>{change.agent_id}</strong>
                  <code>{change.profile_id}</code>
                  <small>
                    {modelStateLabel(change.before)} → {modelStateLabel(change.after)}
                    {change.fallback_profile_id ? `；回退到 ${change.fallback_profile_id}` : ""}
                  </small>
                </li>
              ))}
            </ul>
          </section>
        )}
        {(!isConfiguration || plan.relationship_changes.length > 0) && <section>
          <h3>{isConfiguration ? "Skills 影响" : isAgentSkillPlan ? "生效范围" : agentName ? "Agent 变更" : "关系变化"}</h3>
          {isAgentSkillPlan && compatibleAgentCount > 0 && (
            <p className="mux-asset-review-note">
              只写入一个目录；兼容 Agent 会读取同一份 Skill，不会重复安装。
            </p>
          )}
          {plan.relationship_changes.length === 0 ? <p>无变化</p> : (
            <ul className={isAgentSkillPlan ? "mux-skill-impact-list" : undefined}>
              {plan.relationship_changes.map((change, index) => {
                const isDirect = !isAgentSkillPlan || change.agent_id === agentId;
                const action = change.action === "add"
                  ? isDirect ? "直接添加" : "兼容可见"
                  : isDirect ? "直接移除" : "同步不可见";
                return (
                <li key={`${change.agent_id}:${assetIdentity(change.asset)}:${index}`}>
                  <span data-action={change.action} data-impact={isDirect ? "direct" : "compatible"}>
                    {isAgentSkillPlan ? action : change.action === "add" ? "添加" : "移除"}
                  </span>
                  <strong>{change.agent_id}</strong>
                  <code>{assetLabel(change.asset)}</code>
                </li>
                );
              })}
            </ul>
          )}
        </section>}
        {plan.domain_plan.domain === "agent-configuration"
          && plan.domain_plan.migrated_skill_names.length > 0 && (
            <section>
              <h3>迁移 Skills</h3>
              <p>{plan.domain_plan.migrated_skill_names.join("、")}</p>
            </section>
          )}
        {plan.target_files.length > 0 && (
          <section>
            <h3>{isAgentSkillPlan ? "实际写入位置" : agentName ? "将更新的位置" : "写入目标"}</h3>
            <ul>{plan.target_files.map((path) => <li key={path}><code>{path}</code></li>)}</ul>
          </section>
        )}
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
            允许覆盖上述差异
          </label>
        )}
      </div>
    </DialogShell>
  );
}
