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
      aria-label="旧配置导入提醒"
    >
      <span className="mux-migration-banner-icon" aria-hidden="true">
        <LayersIcon className="w-4 h-4" />
      </span>
      <div className="mux-migration-banner-content">
        <strong>发现 {candidates.length} 项可导入的旧配置</strong>
        <p>整理到 MUX，后续可以统一查看和管理。</p>
        <ul aria-label="待导入配置分类">
          {domains
            .filter(([, count]) => count > 0)
            .map(([label, count]) => (
              <li key={label}>{label} {count}</li>
            ))}
        </ul>
      </div>
      <div className="mux-migration-banner-actions">
        <button type="button" className="btn-ghost" onClick={onLater}>稍后</button>
        <button type="button" className="btn-primary" onClick={onOpen}>去处理</button>
      </div>
    </aside>
  );
}
