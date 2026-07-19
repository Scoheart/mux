import { useId, useRef, type KeyboardEvent, type ReactNode } from "react";

export type AgentResourceTab = "mcps" | "models" | "skills";

const TABS: Array<{ id: AgentResourceTab; label: string }> = [
  { id: "mcps", label: "MCPs" },
  { id: "models", label: "Models" },
  { id: "skills", label: "Skills" },
];

export function AgentResourcePanel({
  value,
  onChange,
  counts,
  children,
}: {
  value: AgentResourceTab;
  onChange: (value: AgentResourceTab) => void;
  counts: Record<AgentResourceTab, number>;
  children: ReactNode;
}) {
  const id = useId();
  const refs = useRef<Array<HTMLButtonElement | null>>([]);
  const selectedIndex = TABS.findIndex((tab) => tab.id === value);
  const panelId = `${id}-panel`;

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
    <section className="mux-agent-resource-panel" aria-labelledby={`${id}-title`}>
      <div className="mux-agent-resource-panel-head">
        <h3 id={`${id}-title`}>Agent 配置</h3>
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
              <span>{tab.label}</span>
              <span>{counts[tab.id]}</span>
            </button>
          ))}
        </div>
      </div>
      <div
        className="mux-agent-resource-panel-body"
        id={panelId}
        role="tabpanel"
        aria-labelledby={`${id}-${TABS[selectedIndex]?.id ?? value}`}
      >
        {children}
      </div>
    </section>
  );
}
