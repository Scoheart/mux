import { useMemo, useState } from "react";
import * as api from "../lib/api";
import type { MigrationCandidate } from "../lib/migration";
import { migrationCounts } from "../lib/migration";
import { AgentGlyph, agentName } from "./brandIcons";
import { CheckIcon, LayersIcon, PackageIcon, SparklesIcon } from "./icons";
import { DialogShell } from "./DialogShell";

type MigrationResult = {
  id: string;
  name: string;
  ok: boolean;
  message: string;
};

export function MigrationDialog({
  candidates,
  onClose,
  onRefresh,
}: {
  candidates: MigrationCandidate[];
  onClose(): void;
  onRefresh(): Promise<void>;
}) {
  const safeIds = useMemo(
    () => new Set(candidates.filter((candidate) => candidate.safe).map((candidate) => candidate.id)),
    [candidates],
  );
  const [selected, setSelected] = useState(safeIds);
  const [busy, setBusy] = useState(false);
  const [results, setResults] = useState<MigrationResult[]>([]);
  const counts = migrationCounts(candidates);
  const selectedItems = candidates.filter(
    (candidate) =>
      candidate.safe
      && selected.has(candidate.id)
      && !results.some((result) => result.id === candidate.id && result.ok),
  );

  const toggle = (id: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const migrate = async () => {
    if (busy || selectedItems.length === 0) return;
    setBusy(true);
    const nextResults: MigrationResult[] = [];
    for (const candidate of selectedItems) {
      let pending: { domain: "mcp" | "model" | "skill"; operationId: string } | null = null;
      try {
        if (candidate.domain === "mcp" && candidate.mcp) {
          const plan = await api.planMcpAdoption({
            asset_key: candidate.mcp.assetKey,
            agent_ids: candidate.agentIds,
            candidate_fingerprints: candidate.mcp.candidateFingerprints,
          });
          pending = { domain: "mcp", operationId: plan.operation_id };
          await api.commitAssetOperation(plan);
        } else if (candidate.domain === "model" && candidate.model) {
          const plan = await api.planModelAdoption({
            candidate_fingerprints: candidate.model.candidateFingerprints,
          });
          pending = { domain: "model", operationId: plan.operation_id };
          await api.commitAssetOperation(plan);
        } else if (candidate.domain === "skill" && candidate.skill) {
          const plan = await api.planSkillImport({
            identity: candidate.skill.identity,
            agent_ids: candidate.agentIds,
            replace_conflicts: false,
          });
          pending = { domain: "skill", operationId: plan.operation_id };
          if (plan.requires_risk_override) {
            throw new Error("Skill 风险状态已变化；请在 Skills 页面单独导入并确认风险。");
          }
          await api.commitSkillImport({
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
            findings_confirmation: null,
          });
        } else {
          throw new Error("迁移候选缺少受管来源。");
        }
        nextResults.push({
          id: candidate.id,
          name: candidate.name,
          ok: true,
          message: "已导入并恢复原使用关系",
        });
        pending = null;
      } catch (reason) {
        if (pending) {
          const cancellation = pending.domain === "mcp" || pending.domain === "model"
            ? api.cancelAssetOperation(pending.operationId)
            : api.cancelSkillOperation(pending.operationId);
          await cancellation.catch(() => undefined);
        }
        const message = formatError(reason);
        nextResults.push({ id: candidate.id, name: candidate.name, ok: false, message });
      }
      setResults([...nextResults]);
    }
    await onRefresh().catch(() => undefined);
    setBusy(false);
  };

  const rows = (domain: "mcp" | "model" | "skill") => candidates.filter((item) => item.domain === domain);
  const toggleDomain = (domainRows: MigrationCandidate[]) => {
    const selectable = domainRows.filter(
      (candidate) =>
        candidate.safe
        && !results.some((result) => result.id === candidate.id && result.ok),
    );
    const allSelected = selectable.length > 0
      && selectable.every((candidate) => selected.has(candidate.id));
    setSelected((current) => {
      const next = new Set(current);
      for (const candidate of selectable) {
        if (allSelected) next.delete(candidate.id);
        else next.add(candidate.id);
      }
      return next;
    });
  };

  return (
    <DialogShell
      kind="review"
      size="lg"
      title="导入旧配置"
      subtitle={`共 ${counts.all} 项 · ${counts.safe} 项可导入 · ${counts.conflicts} 项需先处理`}
      busy={busy}
      onClose={onClose}
      footerStart={results.length > 0 ? (
        <span className="mux-migration-summary">
          成功 {results.filter((item) => item.ok).length} 项，失败 {results.filter((item) => !item.ok).length} 项
        </span>
      ) : null}
      footerEnd={
        <>
          <button type="button" className="btn-ghost" disabled={busy} onClick={onClose}>稍后处理</button>
          <button
            type="button"
            className="btn-primary"
            disabled={busy || selectedItems.length === 0}
            onClick={() => void migrate()}
          >
            {busy ? "正在导入…" : `导入 ${selectedItems.length} 项`}
          </button>
        </>
      }
    >
      <div className="mux-migration-content">
        <p className="mux-migration-intro">
          选择要整理到 MUX 的旧配置；原 Agent 的权限和登录状态不会改变。
        </p>
        {(["mcp", "model", "skill"] as const).map((domain) => {
          const domainRows = rows(domain);
          if (domainRows.length === 0) return null;
          const domainLabel = domain === "mcp" ? "MCP" : domain === "model" ? "Model" : "Skill";
          const selectable = domainRows.filter(
            (candidate) =>
              candidate.safe
              && !results.some((result) => result.id === candidate.id && result.ok),
          );
          const allSelected = selectable.length > 0
            && selectable.every((candidate) => selected.has(candidate.id));
          return (
            <section
              key={domain}
              className="mux-migration-section"
              aria-label={`${domainLabel} 旧配置`}
            >
              <header>
                {domain === "mcp" ? <PackageIcon className="w-4 h-4" /> : domain === "model" ? <LayersIcon className="w-4 h-4" /> : <SparklesIcon className="w-4 h-4" />}
                <strong>{domainLabel}</strong>
                <span>{domainRows.length}</span>
                <button
                  type="button"
                  disabled={busy || selectable.length === 0}
                  aria-label={`${allSelected ? "取消全选" : "全选"} ${domainLabel}`}
                  onClick={() => toggleDomain(domainRows)}
                >
                  {selectable.length === 0 ? "无可导入项" : allSelected ? "取消全选" : "全选"}
                </button>
              </header>
              <ul>
                {domainRows.map((candidate) => {
                  const result = results.find((item) => item.id === candidate.id);
                  return (
                    <li key={candidate.id} data-conflict={!candidate.safe || undefined} data-result={result?.ok ? "success" : result ? "error" : undefined}>
                      <label>
                        <input
                          type="checkbox"
                          checked={candidate.safe && selected.has(candidate.id)}
                          disabled={!candidate.safe || busy || result?.ok}
                          onChange={() => toggle(candidate.id)}
                        />
                        <span className="mux-migration-copy">
                          <strong>{candidate.name}</strong>
                          <small title={candidate.detail}>{candidate.detail}</small>
                          {candidate.conflictReason && <em>{candidate.conflictReason}</em>}
                          {result && <em data-result={result.ok ? "success" : "error"}>{result.message}</em>}
                        </span>
                      </label>
                      <span className="mux-migration-agents" aria-label={`${candidate.agentIds.length} 个 Agent`}>
                        {candidate.agentIds.slice(0, 3).map((agentId) => (
                          <span key={agentId} title={agentName(agentId)}><AgentGlyph id={agentId} size={18} /></span>
                        ))}
                        {candidate.agentIds.length > 3 && <small>+{candidate.agentIds.length - 3}</small>}
                      </span>
                      {result?.ok && <CheckIcon className="mux-migration-check w-4 h-4" />}
                    </li>
                  );
                })}
              </ul>
            </section>
          );
        })}
        {candidates.length === 0 && (
          <div className="mux-migration-empty">
            <CheckIcon className="w-6 h-6" />
            <strong>没有待迁移配置</strong>
            <span>MUX 已统一管理当前支持的全局 MCP、Models 与用户级 Skills。</span>
          </div>
        )}
      </div>
    </DialogShell>
  );
}

function formatError(reason: unknown): string {
  if (typeof reason === "object" && reason !== null && "message" in reason) {
    return String(reason.message);
  }
  return String(reason);
}
