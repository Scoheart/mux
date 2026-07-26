import { useEffect, useRef, useState } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import * as api from "../lib/api";
import type {
  MigrationCandidate,
  MigrationCandidateDetail,
  MigrationConflict,
} from "../lib/migration";
import { migrationCounts } from "../lib/migration";
import type { UnifiedOperationPlan } from "../lib/types";
import type { SupportedLocale } from "../i18n";
import { localizeLegacyText } from "../i18n/legacy";
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
  const { t, i18n } = useTranslation();
  const [busyCandidateId, setBusyCandidateId] = useState<string | null>(null);
  const [review, setReview] = useState<CandidateReview | null>(null);
  const [results, setResults] = useState<MigrationResult[]>([]);
  const reviewRef = useRef<CandidateReview | null>(null);
  const counts = migrationCounts(candidates);
  const busy = busyCandidateId !== null;
  const locale: SupportedLocale = i18n.resolvedLanguage === "zh-CN" ? "zh-CN" : "en-US";
  const localizeSourceText = (value: string) => localizeLegacyText(value, locale);
  const list = (values: string[]) => new Intl.ListFormat(locale, {
    style: "short",
    type: "conjunction",
  }).format(values);
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
      const operation = await planCandidate(
        candidate,
        t("externalConfigurations.missingSource"),
      );
      if (operation.domain === "skill" && operation.plan.requires_risk_override) {
        await api.cancelOperation({
          domain: "skill",
          operation_id: operation.plan.operation_id,
        }).catch(() => undefined);
        throw new Error(t("externalConfigurations.riskChanged"));
      }
      setReview({ candidate, operation });
    } catch (reason) {
      rememberResult(candidate, false, localizeSourceText(formatError(reason)));
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
      rememberResult(pending.candidate, false, localizeSourceText(formatError(reason)));
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
      rememberResult(pending.candidate, true, t("externalConfigurations.managed"));
      await onRefresh().catch(() => undefined);
    } catch (reason) {
      await api.cancelOperation({
        domain: pending.operation.domain,
        operation_id: pending.operation.plan.operation_id,
      }).catch(() => undefined);
      reviewRef.current = null;
      setReview(null);
      rememberResult(pending.candidate, false, localizeSourceText(formatError(reason)));
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
    const detail = formatCandidateDetail(t, review.candidate.detail);
    return (
      <DialogShell
        kind="review"
        size="md"
        title={t("externalConfigurations.reviewTitle", { name: review.candidate.name })}
        subtitle={t("externalConfigurations.reviewSubtitle")}
        busy={busy}
        onClose={requestClose}
        footerEnd={
          <>
            <button type="button" className="btn-ghost" disabled={busy} onClick={() => void cancelReview()}>
              {t("externalConfigurations.back")}
            </button>
            <button
              type="button"
              className="btn-primary"
              disabled={busy || !canCommit(review.operation)}
              onClick={() => void commitReview()}
            >
              {busy
                ? t("externalConfigurations.managing")
                : t("externalConfigurations.confirmManage")}
            </button>
          </>
        }
      >
        <div className="mux-migration-review">
          <dl>
            <div><dt>{t("externalConfigurations.type")}</dt><dd>{domainLabel(review.candidate.domain)}</dd></div>
            <div><dt>{t("externalConfigurations.detectionResult")}</dt><dd>{detail}</dd></div>
            <div>
              <dt>{t("externalConfigurations.affectedAgents")}</dt>
              <dd>{impact.agents.length > 0 ? list(impact.agents) : t("externalConfigurations.none")}</dd>
            </div>
            <div>
              <dt>{t("externalConfigurations.targetLocations")}</dt>
              <dd>{impact.targets.length > 0 ? list(impact.targets) : t("externalConfigurations.centralOnly")}</dd>
            </div>
          </dl>
          {impact.warnings.length > 0 && (
            <div className="mux-migration-review-warnings" role="alert">
              <strong>{t("externalConfigurations.warning")}</strong>
              <ul>{impact.warnings.map((warning) => (
                <li key={warning}>{localizeSourceText(warning)}</li>
              ))}</ul>
            </div>
          )}
          {!canCommit(review.operation) && (
            <p className="mux-migration-review-blocked" role="alert">
              {t("externalConfigurations.blocked")}
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
      title={t("externalConfigurations.dialogTitle")}
      subtitle={t("externalConfigurations.dialogSubtitle", counts)}
      busy={busy}
      onClose={requestClose}
      footerStart={results.length > 0 ? (
        <span className="mux-migration-summary">
          {t("externalConfigurations.dialogSummary", {
            managed: results.filter((item) => item.ok).length,
            failed: results.filter((item) => !item.ok).length,
          })}
        </span>
      ) : null}
      footerEnd={(
        <button type="button" className="btn-ghost" disabled={busy} onClick={requestClose}>
          {t("common.close")}
        </button>
      )}
    >
      <div className="mux-migration-content">
        <p className="mux-migration-intro">
          {t("externalConfigurations.dialogIntro")}
        </p>
        {(["mcp", "model", "skill"] as const).map((domain) => {
          const domainRows = rows(domain);
          if (domainRows.length === 0) return null;
          const label = domainLabel(domain);
          return (
            <section
              key={domain}
              className="mux-migration-section"
              aria-label={t("externalConfigurations.domainSection", { domain: label })}
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
                  const detail = formatCandidateDetail(t, candidate.detail);
                  const conflict = formatMigrationConflict(
                    t,
                    candidate.conflict,
                    localizeSourceText,
                  );
                  return (
                    <li key={candidate.id} data-conflict={!candidate.safe || undefined} data-result={result?.ok ? "success" : result ? "error" : undefined}>
                      <span className="mux-migration-copy">
                        <strong>{candidate.name}</strong>
                        <small title={detail}>{detail}</small>
                        {conflict && <em>{conflict}</em>}
                        {result && <em data-result={result.ok ? "success" : "error"}>{result.message}</em>}
                      </span>
                      <span
                        className="mux-migration-agents"
                        aria-label={t("externalConfigurations.agentCount", {
                          count: candidate.agentIds.length,
                        })}
                      >
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
                          {itemBusy
                            ? t("externalConfigurations.checking")
                            : candidate.safe
                              ? t("externalConfigurations.manage")
                              : t("externalConfigurations.needsAttention")}
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
            <strong>{t("externalConfigurations.emptyTitle")}</strong>
            <span>{t("externalConfigurations.emptyDescription")}</span>
          </div>
        )}
      </div>
    </DialogShell>
  );
}

async function planCandidate(
  candidate: MigrationCandidate,
  missingSourceMessage: string,
): Promise<UnifiedOperationPlan> {
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
  throw new Error(missingSourceMessage);
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
      agents: operation.plan.affected_agent_ids.map((id) => agentName(id)),
      targets: operation.plan.target_files,
      warnings: operation.plan.warnings,
    };
  }
  const agentIds = new Set(operation.plan.targets.flatMap((target) => target.affected_agent_ids));
  return {
    agents: [...agentIds].map((id) => agentName(id)),
    targets: operation.plan.targets.map((target) => target.global_dir),
    warnings: operation.plan.warnings,
  };
}

function formatCandidateDetail(t: TFunction, detail: MigrationCandidateDetail): string {
  if (detail.kind === "model") {
    return t("externalConfigurations.details.model", {
      provider: detail.provider,
      model: detail.model,
      agentCount: detail.agentCount,
      active: detail.activeCount > 0
        ? t("externalConfigurations.details.active", { count: detail.activeCount })
        : "",
    });
  }
  if (detail.kind === "mcp") {
    return t("externalConfigurations.details.mcp", {
      transport: detail.transport,
      agentCount: detail.agentCount,
      disabled: detail.disabledCount > 0
        ? t("externalConfigurations.details.disabled", { count: detail.disabledCount })
        : "",
      mode: t(detail.centralExists
        ? "externalConfigurations.details.inPlace"
        : "externalConfigurations.details.centralCopy"),
    });
  }
  return t("externalConfigurations.details.skill", {
    agentCount: detail.agentCount,
    folderCount: detail.folderCount,
  });
}

function formatMigrationConflict(
  t: TFunction,
  conflict: MigrationConflict | null,
  localizeSourceText: (value: string) => string,
): string | null {
  if (!conflict) return null;
  switch (conflict.kind) {
    case "model_shared_provider_identity":
      return t("externalConfigurations.conflicts.modelSharedProvider");
    case "model_source":
      return localizeSourceText(conflict.reason);
    case "model_credential_or_config":
      return t("externalConfigurations.conflicts.modelCredential");
    case "mcp_drifted":
      return t("externalConfigurations.conflicts.mcpDrifted");
    case "mcp_connection_mismatch":
      return t("externalConfigurations.conflicts.mcpConnection");
    case "skill_central_conflict":
      return t("externalConfigurations.conflicts.skillCentral");
    case "skill_high_risk":
      return t("externalConfigurations.conflicts.skillHighRisk");
    case "skill_missing_audit":
      return t("externalConfigurations.conflicts.skillMissingAudit");
    case "skill_content_mismatch":
      return t("externalConfigurations.conflicts.skillContent");
    case "skill_invalid":
      return t("externalConfigurations.conflicts.skillInvalid");
  }
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
