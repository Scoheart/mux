import { WrenchIcon, BrainIcon } from "./icons";

export type FeatureTab = "mcps" | "models";

interface FeatureTabsProps {
  active: FeatureTab;
  onSelectMcps: () => void;
  onSelectModels: () => void;
}

/** In-app feature tabs — lives in the main column so the left sidebar can
 *  reach the window header. Extensible for future feature areas. */
export function FeatureTabs({ active, onSelectMcps, onSelectModels }: FeatureTabsProps) {
  return (
    <nav className="mux-feature-tabs" aria-label="功能">
      <button
        type="button"
        className="mux-feature-tab"
        data-active={active === "mcps" ? "true" : undefined}
        onClick={onSelectMcps}
      >
        <WrenchIcon />
        <span>MCPs</span>
      </button>
      <button
        type="button"
        className="mux-feature-tab"
        data-active={active === "models" ? "true" : undefined}
        onClick={onSelectModels}
      >
        <BrainIcon />
        <span>Models</span>
        <span className="mux-feature-tab-beta">Beta</span>
      </button>
    </nav>
  );
}
