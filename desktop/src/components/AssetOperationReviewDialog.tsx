import { useState } from "react";
import type { AssetCommandError, AssetOperationPlan, AssetRef } from "../lib/types";
import { assetIdentity } from "../lib/consumption";
import { DialogShell } from "./DialogShell";
import { TrashIcon } from "./icons";

function assetKey(asset: AssetRef) {
  return `${asset.domain}:${assetIdentity(asset)}`;
}

function readableIdentity(value: string) {
  return value
    .replace(/::(?:stdio|http)$/, "")
    .split(/[-_]/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toLocaleUpperCase() + part.slice(1))
    .join(" ");
}

function displayAssetName(asset: AssetRef, names: Record<string, string>) {
  return names[assetKey(asset)] ?? readableIdentity(assetIdentity(asset));
}

function displayAgentName(
  id: string,
  currentId: string | undefined,
  currentName: string | undefined,
  names: Record<string, string>,
) {
  if (id === currentId && currentName) return currentName;
  return names[id] ?? readableIdentity(id);
}

function assetLabel(asset: AssetRef, names: Record<string, string>) {
  const domain = asset.domain === "mcp" ? "MCP" : asset.domain === "model" ? "Model" : "Skill";
  return `${domain} · ${displayAssetName(asset, names)}`;
}

export function assetReviewErrorMessage(
  error: AssetCommandError | string,
  plan: AssetOperationPlan,
) {
  const code = typeof error === "string" ? "" : error.code;
  const message = typeof error === "string" ? error : error.message;
  const domain = plan.domain_plan.domain;
  const normalized = `${code} ${message}`.toLocaleLowerCase();
  if (
    domain === "skill"
    && /only an exact managed skill link can be disabled/i.test(message)
  ) {
    const target = plan.target_files.find((path) => path.trim().length > 0) ?? "路径未知";
    return `无法移除：这不是可安全移除的托管 Skill 链接（${target}）。`;
  }
  if (
    code === "model_consumption_missing"
    || /(?:requested )?model.*(?:not assigned|not added)/i.test(message)
  ) {
    return "该 Model 未添加到此 Agent，无法移除。";
  }
  if (
    code === "mcp_consumption_missing"
    || /mcp.*(?:not assigned|not added)/i.test(message)
  ) {
    return "该 MCP 未分配给此 Agent，无法移除。";
  }
  if (
    code === "skill_consumption_missing"
    || /skill.*(?:not assigned|not added)/i.test(message)
  ) {
    return "该 Skill 未分配给此 Agent，无法移除。";
  }
  const hasRemove = plan.relationship_changes.some((change) => change.action === "remove");
  if (
    hasRemove
    && (
      normalized.includes("conflict")
      || normalized.includes("not assigned")
      || normalized.includes("not added")
      || /^[\[{].*[\]}]$/s.test(message.trim())
    )
  ) {
    const asset = domain === "mcp" ? "MCP" : domain === "model" ? "Model" : "Skill";
    return `当前 ${asset} 状态已变化，无法按此计划移除。请关闭后刷新再试。`;
  }
  return message;
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

interface ConfigurationValues {
  mcpPath: string | null;
  mcpKey: string | null;
  modelPaths: string[];
  skillsGlobalDir: string | null;
  skillsAliasDirs: string[];
}

function configurationPlanValues(plan: AssetOperationPlan): {
  agentId: string;
  before: ConfigurationValues;
  after: ConfigurationValues;
  migratedSkillNames: string[];
} | null {
  const domainPlan = plan.domain_plan;
  if (domainPlan.domain === "agent-configuration") {
    const project = (value: typeof domainPlan.before): ConfigurationValues => ({
      mcpPath: value.mcp_path,
      mcpKey: value.mcp_key ?? null,
      modelPaths: value.model_paths,
      skillsGlobalDir: value.skills_global_dir,
      skillsAliasDirs: value.skills_alias_dirs ?? [],
    });
    return {
      agentId: domainPlan.agent_id,
      before: project(domainPlan.before),
      after: project(domainPlan.after),
      migratedSkillNames: domainPlan.migrated_skill_names,
    };
  }
  if (domainPlan.domain === "agent-capabilities") {
    const project = (value: typeof domainPlan.before): ConfigurationValues => ({
      mcpPath: value.mcp?.path ?? null,
      mcpKey: value.mcp?.key ?? null,
      modelPaths: value.model?.paths ?? [],
      skillsGlobalDir: value.skill?.global_dir ?? null,
      skillsAliasDirs: value.skill?.alias_dirs ?? [],
    });
    return {
      agentId: domainPlan.agent_id,
      before: project(domainPlan.before),
      after: project(domainPlan.after),
      migratedSkillNames: domainPlan.migrated_skill_names,
    };
  }
  return null;
}

function configurationChanges(plan: AssetOperationPlan) {
  const configuration = configurationPlanValues(plan);
  if (!configuration) return [];
  const { before, after } = configuration;
  const rows: Array<{ label: string; before: string; after: string }> = [];
  if (before.mcpPath !== after.mcpPath) {
    rows.push({
      label: "MCP 文件路径",
      before: before.mcpPath ?? "未接入",
      after: after.mcpPath ?? "未接入",
    });
  }
  const beforeMcpKey = before.mcpKey ?? "";
  const afterMcpKey = after.mcpKey ?? "";
  if (beforeMcpKey !== afterMcpKey) {
    rows.push({ label: "MCP 配置键", before: beforeMcpKey, after: afterMcpKey });
  }
  const beforeModels = before.modelPaths.join(" · ");
  const afterModels = after.modelPaths.join(" · ");
  if (beforeModels !== afterModels) {
    rows.push({ label: "Model", before: beforeModels, after: afterModels });
  }
  const beforeSkills = [before.skillsGlobalDir, ...before.skillsAliasDirs]
    .filter(Boolean).join(" · ");
  const afterSkills = [after.skillsGlobalDir, ...after.skillsAliasDirs]
    .filter(Boolean).join(" · ");
  if (beforeSkills !== afterSkills) {
    rows.push({
      label: "Skills",
      before: beforeSkills || "未接入",
      after: afterSkills || "未接入",
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
  agentDisplayNames = {},
  assetDisplayNames = {},
  onCommit,
  onCancel,
}: {
  plan: AssetOperationPlan;
  busy: boolean;
  error?: AssetCommandError | string | null;
  agentId?: string;
  agentName?: string;
  agentDisplayNames?: Record<string, string>;
  assetDisplayNames?: Record<string, string>;
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
  const configurationPlan = configurationPlanValues(plan);
  const skillsPathChanged = configurationPlan !== null
    && (configurationPlan.before.skillsGlobalDir !== configurationPlan.after.skillsGlobalDir
      || JSON.stringify(configurationPlan.before.skillsAliasDirs)
        !== JSON.stringify(configurationPlan.after.skillsAliasDirs));
  const sharedConfigurationReaderCount = isConfiguration
    && configurationPlan
    && skillsPathChanged
    ? plan.affected_agent_ids.filter((id) => id !== configurationPlan.agentId).length
    : 0;
  const configChanges = configurationChanges(plan);
  const mcpLocationChanged = configurationPlan !== null
    && (configurationPlan.before.mcpPath !== configurationPlan.after.mcpPath
      || configurationPlan.before.mcpKey !== configurationPlan.after.mcpKey);
  const modelLocationChanged = configurationPlan !== null
    && (configurationPlan.before.modelPaths.length !== configurationPlan.after.modelPaths.length
      || configurationPlan.before.modelPaths.some(
        (path, index) => path !== configurationPlan.after.modelPaths[index],
      ));
  const hasAdd = plan.relationship_changes.some((change) => change.action === "add");
  const hasRemove = plan.relationship_changes.some((change) => change.action === "remove");
  const isRemoveOnly = hasRemove && !hasAdd;
  const reviewError = error ? assetReviewErrorMessage(error, plan) : null;
  const agentCopy = agentName && plan.kind === "set-consumption" ? agentActionCopy(plan) : null;
  const title = isConfiguration ? "确认修改配置" : agentCopy?.title ?? (plan.kind === "update-asset"
    ? "确认更改"
    : plan.kind === "delete-asset"
      ? "确认删除"
      : "确认更新配置");
  const commitLabel = isConfiguration ? "保存配置" : agentCopy?.commit ?? (plan.kind === "delete-asset" ? "删除" : "应用更改");
  const subtitle = agentName
    ? compatibleAgentCount > 0
      ? `${agentName} · 同一目录也被 ${compatibleAgentCount} 个 Agent 读取`
      : sharedConfigurationReaderCount > 0
        ? `${agentName} · Skills 目录变更涉及 ${sharedConfigurationReaderCount} 个其他 Agent`
      : !isConfiguration && plan.affected_agent_ids.length > 1
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
          : reviewError
            ? <div className="mux-asset-review-error" role="alert">
                <strong>操作未完成</strong>
                <span>{reviewError}</span>
              </div>
            : null}
      footerEnd={
        <>
          <button type="button" className="btn-ghost" disabled={busy} onClick={() => void onCancel()}>取消</button>
          <button
            type="button"
            className={isRemoveOnly || plan.kind === "delete-asset" ? "btn-danger" : "btn-primary"}
            disabled={busy || !plan.can_commit || (plan.requires_conflict_confirmation && !replaceDrift)}
            onClick={() => void onCommit(plan.requires_conflict_confirmation ? plan.candidate_hash : undefined)}
          >
            {!busy && (isRemoveOnly || plan.kind === "delete-asset") && <TrashIcon className="w-4 h-4" />}
            {busy
              ? (isConfiguration ? "保存中…" : agentCopy?.busy ?? "处理中…")
              : reviewError && isRemoveOnly
                ? `重试${commitLabel}`
                : commitLabel}
          </button>
        </>
      }
    >
      <div className="mux-review-content mux-asset-review">
        {isRemoveOnly && (
          <section className="mux-asset-review-summary">
            <h3>影响摘要</h3>
            {plan.relationship_changes
              .filter((change) => change.action === "remove")
              .map((change, index) => (
                <p key={`${change.agent_id}:${assetIdentity(change.asset)}:${index}`}>
                  {isAgentSkillPlan && change.agent_id !== agentId ? (
                    <>
                      共用目录将同步影响
                      <strong>
                        {displayAgentName(
                          change.agent_id,
                          agentId,
                          agentName,
                          agentDisplayNames,
                        )}
                      </strong>
                      的 Skill 可见性。
                    </>
                  ) : (
                    <>
                      将从
                      <strong>
                        {displayAgentName(
                          change.agent_id,
                          agentId,
                          agentName,
                          agentDisplayNames,
                        )}
                      </strong>
                      移除 {assetLabel(change.asset, assetDisplayNames)}。
                    </>
                  )}
                </p>
              ))}
          </section>
        )}
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
            {mcpLocationChanged && (
              <p className="mux-asset-review-note">
                只更新 MUX 后续使用的 MCP 配置位置；旧文件不会删除，现有 MCP 配置不会复制到新位置。
              </p>
            )}
            {modelLocationChanged && (
              <p className="mux-asset-review-note">
                只更新 MUX 后续使用的 Model 配置位置；旧文件不会删除，现有 Model 配置不会复制到新位置。
              </p>
            )}
          </section>
        )}
        {plan.central_changes.length > 0 && (
          <section>
            <h3>资源变化</h3>
            <ul>
              {plan.central_changes.map((change) => (
                <li key={`${change.action}:${assetIdentity(change.asset)}`}>
                  <span data-action={change.action}>
                    {change.action === "create" ? "创建" : change.action === "update" ? "更新" : "删除"}
                  </span>
                  <code>{assetLabel(change.asset, assetDisplayNames)}</code>
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
                  <strong>
                    {displayAgentName(change.agent_id, agentId, agentName, agentDisplayNames)}
                  </strong>
                  <code>
                    {displayAssetName(
                      { domain: "model", profile_id: change.profile_id },
                      assetDisplayNames,
                    )}
                  </code>
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
                <li
                  className="mux-asset-review-relationship"
                  key={`${change.agent_id}:${assetIdentity(change.asset)}:${index}`}
                >
                  <span data-action={change.action} data-impact={isDirect ? "direct" : "compatible"}>
                    {isAgentSkillPlan ? action : change.action === "add" ? "添加" : "移除"}
                  </span>
                  <span className="mux-asset-review-relationship-copy">
                    <strong>
                      {displayAgentName(change.agent_id, agentId, agentName, agentDisplayNames)}
                    </strong>
                    <small>{assetLabel(change.asset, assetDisplayNames)}</small>
                  </span>
                </li>
                );
              })}
            </ul>
          )}
        </section>}
        {configurationPlan
          && configurationPlan.migratedSkillNames.length > 0 && (
            <section>
              <h3>迁移 Skills</h3>
              <p>{configurationPlan.migratedSkillNames.join("、")}</p>
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
