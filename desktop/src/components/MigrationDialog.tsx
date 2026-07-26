import { useEffect, useRef, useState } from "react";
import * as api from "../lib/api";
import type { MigrationCandidate } from "../lib/migration";
import { migrationCounts } from "../lib/migration";
import type { UnifiedOperationPlan } from "../lib/types";
import { AgentGlyph, agentName } from "./brandIcons";
import { CheckIcon, LayersIcon, PackageIcon, SparklesIcon } from "./icons";
import { DialogShell } from "./DialogShell";

type MigrationResult = {
  id: string;
  ok: boolean;
  message: string;
};

type CandidateReview = {
  candidate: MigrationCandidate;
  operation: UnifiedOperationPlan;
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
  const [busyCandidateId, setBusyCandidateId] = useState<string | null>(null);
  const [review, setReview] = useState<CandidateReview | null>(null);
  const [results, setResults] = useState<MigrationResult[]>([]);
  const reviewRef = useRef<CandidateReview | null>(null);
  const counts = migrationCounts(candidates);
  const busy = busyCandidateId !== null;
  const rows = (domain: "mcp" | "model" | "skill") =>
    candidates.filter((item) => item.domain === domain);

  useEffect(() => {
    reviewRef.current = review;
  }, [review]);

  useEffect(() => () => {
    const pending = reviewRef.current;
    if (pending) {
      void api.cancelOperation({
        domain: pending.operation.domain,
        operation_id: pending.operation.plan.operation_id,
      }).catch(() => undefined);
    }
  }, []);

  const rememberResult = (candidate: MigrationCandidate, ok: boolean, message: string) => {
    setResults((current) => [
      ...current.filter((item) => item.id !== candidate.id),
      { id: candidate.id, ok, message },
    ]);
  };

  const prepare = async (candidate: MigrationCandidate) => {
    if (busy || review || !candidate.safe) return;
    setBusyCandidateId(candidate.id);
    try {
      const operation = await planCandidate(candidate);
      if (operation.domain === "skill" && operation.plan.requires_risk_override) {
        await api.cancelOperation({
          domain: "skill",
          operation_id: operation.plan.operation_id,
        }).catch(() => undefined);
        throw new Error("Skill 风险状态已变化；请在 Skills 页面单独导入并确认风险。");
      }
      setReview({ candidate, operation });
    } catch (reason) {
      rememberResult(candidate, false, formatError(reason));
    } finally {
      setBusyCandidateId(null);
    }
  };

  const cancelReview = async () => {
    if (!review || busy) return;
    const pending = review;
    setBusyCandidateId(pending.candidate.id);
    try {
      await api.cancelOperation({
        domain: pending.operation.domain,
        operation_id: pending.operation.plan.operation_id,
      });
      reviewRef.current = null;
      setReview(null);
    } catch (reason) {
      rememberResult(pending.candidate, false, formatError(reason));
    } finally {
      setBusyCandidateId(null);
    }
  };

  const commitReview = async () => {
    if (!review || busy || !canCommit(review.operation)) return;
    const pending = review;
    setBusyCandidateId(pending.candidate.id);
    try {
      await commitCandidate(pending.operation);
      reviewRef.current = null;
      setReview(null);
      rememberResult(pending.candidate, true, "已由 MUX 管理");
      await onRefresh().catch(() => undefined);
    } catch (reason) {
      await api.cancelOperation({
        domain: pending.operation.domain,
        operation_id: pending.operation.plan.operation_id,
      }).catch(() => undefined);
      reviewRef.current = null;
      setReview(null);
      rememberResult(pending.candidate, false, formatError(reason));
    } finally {
      setBusyCandidateId(null);
    }
  };

  const requestClose = () => {
    if (busy) return;
    if (review) {
      void cancelReview().then(onClose);
      return;
    }
    onClose();
  };

  if (review) {
    const impact = reviewImpact(review.operation);
    return (
      <DialogShell
        kind="review"
        size="md"
        title={`确认让 MUX 管理 ${review.candidate.name}`}
        subtitle="这里只处理当前这一项；MUX 不会顺带导入其它已识别配置。"
        busy={busy}
        onClose={requestClose}
        footerEnd={
          <>
            <button type="button" className="btn-ghost" disabled={busy} onClick={() => void cancelReview()}>
              返回
            </button>
            <button
              type="button"
              className="btn-primary"
              disabled={busy || !canCommit(review.operation)}
              onClick={() => void commitReview()}
            >
              {busy ? "正在处理…" : "确认让 MUX 管理"}
            </button>
          </>
        }
      >
        <div className="mux-migration-review">
          <dl>
            <div><dt>类型</dt><dd>{domainLabel(review.candidate.domain)}</dd></div>
            <div><dt>识别结果</dt><dd>{review.candidate.detail}</dd></div>
            <div><dt>影响 Agent</dt><dd>{impact.agents || "无"}</dd></div>
            <div><dt>目标位置</dt><dd>{impact.targets || "仅更新 MUX 中央配置"}</dd></div>
          </dl>
          {impact.warnings.length > 0 && (
            <div className="mux-migration-review-warnings" role="alert">
              <strong>需要注意</strong>
              <ul>{impact.warnings.map((warning) => <li key={warning}>{warning}</li>)}</ul>
            </div>
          )}
          {!canCommit(review.operation) && (
            <p className="mux-migration-review-blocked" role="alert">
              当前配置已经变化或存在冲突，不能直接纳管。请返回修复后重新扫描。
            </p>
          )}
        </div>
      </DialogShell>
    );
  }

  return (
    <DialogShell
      kind="review"
      size="lg"
      title="已识别的外部配置"
      subtitle={`共 ${counts.all} 项 · ${counts.safe} 项可逐项管理 · ${counts.conflicts} 项需先处理`}
      busy={busy}
      onClose={requestClose}
      footerStart={results.length > 0 ? (
        <span className="mux-migration-summary">
          已管理 {results.filter((item) => item.ok).length} 项，失败 {results.filter((item) => !item.ok).length} 项
        </span>
      ) : null}
      footerEnd={<button type="button" className="btn-ghost" disabled={busy} onClick={requestClose}>关闭</button>}
    >
      <div className="mux-migration-content">
        <p className="mux-migration-intro">
          MUX 只识别这些 Agent 配置，不会自动导入。请检查每一项，并单独决定是否交给 MUX 管理。
        </p>
        {(["mcp", "model", "skill"] as const).map((domain) => {
          const domainRows = rows(domain);
          if (domainRows.length === 0) return null;
          const label = domainLabel(domain);
          return (
            <section
              key={domain}
              className="mux-migration-section"
              aria-label={`${label} 外部配置`}
            >
              <header>
                {domain === "mcp" ? <PackageIcon className="w-4 h-4" /> : domain === "model" ? <LayersIcon className="w-4 h-4" /> : <SparklesIcon className="w-4 h-4" />}
                <strong>{label}</strong>
                <span>{domainRows.length}</span>
              </header>
              <ul>
                {domainRows.map((candidate) => {
                  const result = results.find((item) => item.id === candidate.id);
                  const itemBusy = busyCandidateId === candidate.id;
                  return (
                    <li key={candidate.id} data-conflict={!candidate.safe || undefined} data-result={result?.ok ? "success" : result ? "error" : undefined}>
                      <span className="mux-migration-copy">
                        <strong>{candidate.name}</strong>
                        <small title={candidate.detail}>{candidate.detail}</small>
                        {candidate.conflictReason && <em>{candidate.conflictReason}</em>}
                        {result && <em data-result={result.ok ? "success" : "error"}>{result.message}</em>}
                      </span>
                      <span className="mux-migration-agents" aria-label={`${candidate.agentIds.length} 个 Agent`}>
                        {candidate.agentIds.slice(0, 3).map((agentId) => (
                          <span key={agentId} title={agentName(agentId)}><AgentGlyph id={agentId} size={18} /></span>
                        ))}
                        {candidate.agentIds.length > 3 && <small>+{candidate.agentIds.length - 3}</small>}
                      </span>
                      {result?.ok ? (
                        <CheckIcon className="mux-migration-check w-4 h-4" />
                      ) : (
                        <button
                          type="button"
                          className="btn-secondary mux-migration-manage"
                          disabled={busy || !candidate.safe}
                          onClick={() => void prepare(candidate)}
                        >
                          {itemBusy ? "正在检查…" : candidate.safe ? "让 MUX 管理" : "需先处理"}
                        </button>
                      )}
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
            <strong>没有已识别的外部配置</strong>
            <span>当前支持的 MCP、Models 与用户级 Skills 已由 MUX 管理，或尚未在 Agent 中配置。</span>
          </div>
        )}
      </div>
    </DialogShell>
  );
}

async function planCandidate(candidate: MigrationCandidate): Promise<UnifiedOperationPlan> {
  if (candidate.domain === "mcp" && candidate.mcp) {
    return api.planOperation({
      operation: "adopt_mcp",
      request: {
        asset_key: candidate.mcp.assetKey,
        agent_ids: candidate.agentIds,
        candidate_fingerprints: candidate.mcp.candidateFingerprints,
      },
    });
  }
  if (candidate.domain === "model" && candidate.model) {
    return api.planOperation({
      operation: "adopt_model",
      request: {
        candidate_fingerprints: candidate.model.candidateFingerprints,
      },
    });
  }
  if (candidate.domain === "skill" && candidate.skill) {
    return api.planOperation({
      operation: "adopt_skill",
      request: {
        identity: candidate.skill.identity,
        agent_ids: candidate.agentIds,
        replace_conflicts: false,
      },
    });
  }
  throw new Error("识别结果缺少可管理的来源。");
}

async function commitCandidate(operation: UnifiedOperationPlan) {
  if (operation.domain === "asset") {
    return api.commitOperation({
      domain: "asset",
      request: {
        operation_id: operation.plan.operation_id,
        candidate_hash: operation.plan.candidate_hash,
        conflict_confirmation: null,
      },
    });
  }
  return api.commitOperation({
    domain: "skill",
    kind: "import",
    request: {
      operation_id: operation.plan.operation_id,
      candidate_hash: operation.plan.candidate_hash,
      findings_confirmation: null,
    },
  });
}

function canCommit(operation: UnifiedOperationPlan): boolean {
  return operation.domain === "skill"
    ? !operation.plan.requires_risk_override
    : operation.plan.can_commit && !operation.plan.requires_conflict_confirmation;
}

function reviewImpact(operation: UnifiedOperationPlan) {
  if (operation.domain === "asset") {
    return {
      agents: operation.plan.affected_agent_ids.map((id) => agentName(id)).join("、"),
      targets: operation.plan.target_files.join("、"),
      warnings: operation.plan.warnings,
    };
  }
  const agentIds = new Set(operation.plan.targets.flatMap((target) => target.affected_agent_ids));
  return {
    agents: [...agentIds].map((id) => agentName(id)).join("、"),
    targets: operation.plan.targets.map((target) => target.global_dir).join("、"),
    warnings: operation.plan.warnings,
  };
}

function domainLabel(domain: MigrationCandidate["domain"]) {
  return domain === "mcp" ? "MCP" : domain === "model" ? "Model" : "Skill";
}

function formatError(reason: unknown): string {
  if (typeof reason === "object" && reason !== null && "message" in reason) {
    return String(reason.message);
  }
  return String(reason);
}
