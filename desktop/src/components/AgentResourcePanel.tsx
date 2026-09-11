import { createContext, useContext, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { LayersIcon, PackageIcon, SlidersIcon, SparklesIcon } from "./icons";

const ToolbarTarget = createContext<HTMLDivElement | null | undefined>(undefined);

export function AgentResourceActions({ children }: { children: ReactNode }) {
  const target = useContext(ToolbarTarget);
  if (target === undefined) return <>{children}</>;
  return target ? createPortal(children, target) : null;
}

export type AgentResourceTab = "mcps" | "models" | "skills";

const TABS: Array<{ id: AgentResourceTab; label: string; icon: typeof PackageIcon }> = [
  { id: "mcps", label: "MCPs", icon: PackageIcon },
  { id: "models", label: "Models", icon: LayersIcon },
  { id: "skills", label: "Skills", icon: SparklesIcon },
];

export function AgentResourcePanel({
  value,
  onChange,
  counts,
  children,
  configuration,
}: {
  value: AgentResourceTab;
  onChange: (value: AgentResourceTab) => void;
  counts: Record<AgentResourceTab, number>;
  children: ReactNode;
  configuration?: ReactNode;
}) {
  const id = useId();
  const refs = useRef<Array<HTMLButtonElement | null>>([]);
  const selectedIndex = TABS.findIndex((tab) => tab.id === value);
  const panelId = `${id}-panel`;
  const [toolbarTarget, setToolbarTarget] = useState<HTMLDivElement | null>(null);
  const [configurationOpen, setConfigurationOpen] = useState(false);

  const handleKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + TABS.length) % TABS.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % TABS.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = TABS.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    onChange(TABS[nextIndex].id);
    refs.current[nextIndex]?.focus();
  };

  return (
    <ToolbarTarget.Provider value={toolbarTarget}><section className="mux-agent-resource-panel" aria-label="Agent 资源与配置">
      <div className="mux-agent-resource-panel-head">
        <div className="mux-agent-resource-tabs" role="tablist" aria-label="Agent 资源">
          {TABS.map((tab, index) => (
            <button
              key={tab.id}
              ref={(element) => { refs.current[index] = element; }}
              type="button"
              role="tab"
              id={`${id}-${tab.id}`}
              aria-controls={panelId}
              aria-selected={tab.id === value}
              tabIndex={tab.id === value ? 0 : -1}
              data-active={tab.id === value ? "true" : undefined}
              onClick={() => onChange(tab.id)}
              onKeyDown={(event) => handleKeyDown(event, index)}
            >
              <span className="mux-agent-resource-tab-icon" aria-hidden="true"><tab.icon className="w-4 h-4" /></span>
              <span>{tab.label}</span>
              <span>{counts[tab.id]}</span>
            </button>
          ))}
        </div>
        <div className="mux-agent-toolbar-actions">
          <div className="mux-agent-toolbar-target" ref={setToolbarTarget} />
          {configuration && <button type="button" className="mux-agent-config-toggle btn-secondary"
            aria-expanded={configurationOpen} aria-controls={`${id}-configuration`}
            onClick={() => setConfigurationOpen((open) => !open)}>
            <SlidersIcon className="w-3.5 h-3.5" />配置
          </button>}
        </div>
      </div>
      {configuration && <div id={`${id}-configuration`} className="mux-agent-active-config" hidden={!configurationOpen}>
        {configuration}
      </div>}
      <div
        className="mux-agent-resource-panel-body"
        id={panelId}
        role="tabpanel"
        aria-labelledby={`${id}-${TABS[selectedIndex]?.id ?? value}`}
      >
        {children}
      </div>
    </section></ToolbarTarget.Provider>
  );
}
