import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  AssetCommandError,
  MigrationResolutionStrategy,
  MigrationReview,
} from "../lib/types";
import { DialogShell } from "./DialogShell";

function actionCandidate(
  review: MigrationReview,
  strategy: MigrationResolutionStrategy,
) {
  return review.actions.find((action) => action.strategy === strategy);
}

export function ModelMigrationReviewDialog({
  review,
  busy,
  error,
  onResolve,
  onLater,
}: {
  review: MigrationReview;
  busy: boolean;
  error: AssetCommandError | null;
  onResolve(
    strategy: MigrationResolutionStrategy,
    candidateHash?: string,
  ): Promise<unknown> | unknown;
  onLater(): void;
}) {
  const { t } = useTranslation();
  const [strategy, setStrategy] = useState<"use_mux" | "keep_agent" | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const selected = useMemo(
    () => strategy ? actionCandidate(review, strategy) : undefined,
    [review, strategy],
  );
  const choose = (next: "use_mux" | "keep_agent") => {
    setStrategy(next);
    setConfirmed(false);
  };

  return (
    <DialogShell
      kind="review"
      size="lg"
      title={t("migration.title")}
      subtitle={t("migration.schema", {
        from: review.source_schema_version,
        to: review.target_schema_version,
      })}
      busy={busy}
      closeLabel={t("migration.later")}
      onClose={onLater}
      status={error ? (
        <div className="mux-asset-review-error" role="alert">
          <strong>{t("migration.notCompleted")}</strong>
          <span>
            {error.code === "migration_review_stale"
              ? t("migration.stale")
              : t("migration.failed", { code: error.code })}
          </span>
        </div>
      ) : null}
      footerStart={
        <button
          type="button"
          className="btn-secondary"
          disabled={busy}
          onClick={() => void onResolve("recheck")}
        >
          {t("migration.recheck")}
        </button>
      }
      footerEnd={
        <>
          <button type="button" className="btn-ghost" disabled={busy} onClick={onLater}>
            {t("migration.later")}
          </button>
          <button
            type="button"
            className="btn-primary"
            disabled={busy || !selected || !confirmed}
            onClick={() => selected && void onResolve(strategy!, selected.plan.candidate_hash)}
          >
            {busy ? t("migration.continuing") : t("migration.continue")}
          </button>
        </>
      }
    >
      <div className="mux-migration-review">
        <p className="mux-migration-intro">{t("migration.intro")}</p>

        <section>
          <h3>{t("migration.conflicts")}</h3>
          <div className="mux-migration-blockers">
            {review.blockers.map((blocker) => (
              <article key={`${blocker.agent_id}:${blocker.profile_id}`}>
                <header>
                  <strong>{blocker.agent_name}</strong>
                  <code>{blocker.agent_id}</code>
                </header>
                <dl>
                  <div>
                    <dt>{t("migration.target")}</dt>
                    <dd>{blocker.target_files.length > 0
                      ? blocker.target_files.map((path) => <code key={path}>{path}</code>)
                      : t("migration.targetUnknown")}</dd>
                  </div>
                  <div>
                    <dt>{t("migration.conflictType")}</dt>
                    <dd>{blocker.reason === "model_owned_fields_drift"
                      ? t("migration.ownedFieldsDrift")
                      : t("migration.targetMissing")}</dd>
                  </div>
                  <div>
                    <dt>{t("migration.modelChange")}</dt>
                    <dd><code>{blocker.before.profile_id}</code><span>→</span><code>{blocker.after.profile_id}</code></dd>
                  </div>
                  {strategy === "keep_agent" && (
                    <div>
                      <dt>{t("migration.relationshipsReleased")}</dt>
                      <dd>{blocker.keep_agent_released_profile_ids.map((profileId) => (
                        <code key={profileId}>{profileId}</code>
                      ))}</dd>
                    </div>
                  )}
                  <div>
                    <dt>{t("migration.managedFields")}</dt>
                    <dd>{blocker.mux_owned_field_categories.join(" · ")}</dd>
                  </div>
                  <div>
                    <dt>{t("migration.keychain")}</dt>
                    <dd>{blocker.migrates_keychain_reference
                      ? strategy === "use_mux"
                        ? t("migration.keychainMigrated")
                        : t("migration.keychainPreserved")
                      : t("migration.keychainUnchanged")}</dd>
                  </div>
                  <div>
                    <dt>{t("migration.session")}</dt>
                    <dd>{blocker.agent_restart_recommended
                      ? t("migration.sessionRestart")
                      : t("migration.sessionNoRestart")}</dd>
                  </div>
                </dl>
              </article>
            ))}
          </div>
        </section>

        <section>
          <h3>{t("migration.choose")}</h3>
          <div className="mux-migration-actions" role="radiogroup" aria-label={t("migration.choose")}>
            <button
              type="button"
              role="radio"
              aria-checked={strategy === "use_mux"}
              data-selected={strategy === "use_mux" || undefined}
              onClick={() => choose("use_mux")}
              disabled={busy}
            >
              <strong>{t("migration.useMux")}</strong>
              <span>{t("migration.useMuxImpact")}</span>
            </button>
            <button
              type="button"
              role="radio"
              aria-checked={strategy === "keep_agent"}
              data-selected={strategy === "keep_agent" || undefined}
              onClick={() => choose("keep_agent")}
              disabled={busy}
            >
              <strong>{t("migration.keepAgent")}</strong>
              <span>{t("migration.keepAgentImpact")}</span>
            </button>
          </div>
          {strategy && (
            <label className="mux-migration-confirmation">
              <input
                type="checkbox"
                checked={confirmed}
                onChange={(event) => setConfirmed(event.target.checked)}
                disabled={busy}
              />
              <span>{strategy === "use_mux"
                ? t("migration.confirmUseMux")
                : t("migration.confirmKeepAgent", {
                    fallback: review.blockers
                      .map((blocker) => blocker.keep_agent_fallback_profile_id)
                      .filter(Boolean)
                      .join(", ") || t("migration.noFallback"),
                  })}</span>
            </label>
          )}
        </section>

        <p className="mux-migration-security">{t("migration.security")}</p>
      </div>
    </DialogShell>
  );
}
