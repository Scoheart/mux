import { useTranslation } from "react-i18next";
import type { MigrationCandidate } from "../lib/migration";
import { migrationCounts } from "../lib/migration";
import { LayersIcon } from "./icons";

export function MigrationBanner({
  candidates,
  onLater,
  onOpen,
}: {
  candidates: MigrationCandidate[];
  onLater(): void;
  onOpen(): void;
}) {
  const { t } = useTranslation();
  if (candidates.length === 0) return null;
  const counts = migrationCounts(candidates);
  const domains = [
    ["MCP", counts.mcp],
    ["Model", counts.model],
    ["Skill", counts.skill],
  ] as const;

  return (
    <aside
      className="mux-migration-banner"
      role="status"
      aria-label={t("externalConfigurations.bannerLabel")}
    >
      <span className="mux-migration-banner-icon" aria-hidden="true">
        <LayersIcon className="w-4 h-4" />
      </span>
      <div className="mux-migration-banner-content">
        <strong>{t("externalConfigurations.bannerTitle", { count: candidates.length })}</strong>
        <p>{t("externalConfigurations.bannerDescription")}</p>
        <ul aria-label={t("externalConfigurations.categoryLabel")}>
          {domains
            .filter(([, count]) => count > 0)
            .map(([label, count]) => (
              <li key={label}>{label} {count}</li>
            ))}
        </ul>
      </div>
      <div className="mux-migration-banner-actions">
        <button type="button" className="btn-ghost" onClick={onLater}>{t("externalConfigurations.later")}</button>
        <button type="button" className="btn-primary" onClick={onOpen}>{t("externalConfigurations.resolve")}</button>
      </div>
    </aside>
  );
}
