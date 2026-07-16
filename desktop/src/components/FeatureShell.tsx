import type { ReactNode } from "react";
import { FeatureTabs, type FeatureTab } from "./FeatureTabs";
import { stickyHeaderStyle } from "./ui";

interface FeatureShellProps {
  active: FeatureTab;
  onSelectMcps: () => void;
  onSelectModels: () => void;
  /** Left column — same chrome on every feature tab. Omit for full-bleed pages. */
  sidebar?: ReactNode;
  /** Optional row under the feature tabs (search / actions). */
  toolbar?: ReactNode;
  children: ReactNode;
}

/** Shared two-pane chrome for MCPs / Models / future feature tabs.
 *  Switching tabs only swaps sidebar + body content — layout & colors stay put. */
export function FeatureShell({
  active,
  onSelectMcps,
  onSelectModels,
  sidebar,
  toolbar,
  children,
}: FeatureShellProps) {
  return (
    <div className="mux-feature-shell">
      {sidebar}
      <div className="mux-feature-main">
        <div className="mux-feature-chrome sticky top-0 z-10" style={stickyHeaderStyle}>
          <FeatureTabs
            active={active}
            onSelectMcps={onSelectMcps}
            onSelectModels={onSelectModels}
          />
          {toolbar}
        </div>
        <div className="mux-feature-body">{children}</div>
      </div>
    </div>
  );
}
