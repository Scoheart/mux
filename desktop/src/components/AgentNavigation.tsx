import { useMemo, useRef, useState } from "react";
import type { AgentInfo } from "../lib/types";
import { buildAgentPickerSections } from "../lib/pinnedAgents";
import { usePinnedAgents } from "../hooks/usePinnedAgents";
import { AgentGlyph } from "./brandIcons";
import { ChevronDownIcon, PackageIcon } from "./icons";
import { PinnedAgentDock } from "./PinnedAgentDock";
import { AgentHandPicker } from "./AgentHandPicker";

export function AgentNavigation({ agents, selectedAgentId, onSelectAgent, onAddAgent }: {
  agents: AgentInfo[];
  selectedAgentId: string | null;
  onSelectAgent(id: string): void;
  onAddAgent?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [announcement, setAnnouncement] = useState("");
  const anchorRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const pinned = usePinnedAgents();
  const sections = useMemo(() => buildAgentPickerSections(agents, pinned.agentIds, ""), [agents, pinned.agentIds]);
  const pinnedIds = useMemo(() => sections.pinned.map(({ id }) => id), [sections]);
  const available = useMemo(() => [...sections.pinned, ...sections.available], [sections]);
  const selected = agents.find(({ id }) => id === selectedAgentId);
  const saveOrder = async (ids: string[], movedId: string) => {
    if (!pinned.ready || pinned.saving || ids.join("\0") === pinnedIds.join("\0")) return;
    if (await pinned.commit(ids)) {
      setAnnouncement(`${agents.find(({ id }) => id === movedId)?.name ?? movedId} 已移动到第 ${ids.indexOf(movedId) + 1} 位`);
    }
  };

  return <div className="mux-agent-navigation">
    <span className="sr-only" aria-live="polite">{announcement}</span>
    <div className="mux-agent-picker-cluster" ref={anchorRef}>
      {pinnedIds.length > 0 && <PinnedAgentDock agents={agents} ids={pinnedIds} selectedId={selectedAgentId}
        disabled={!pinned.ready || pinned.saving} onSelect={onSelectAgent} onReorder={(ids, id) => void saveOrder(ids, id)} />}
      <div className="mux-agent-picker-anchor">
        <button ref={triggerRef} type="button" className="mux-agent-picker-trigger" data-active={selected ? "true" : undefined}
          data-open={open ? "true" : undefined} aria-haspopup="dialog" aria-expanded={open}
          aria-label={selected?.name ?? "选择 Agent"} title={selected?.name ?? "选择 Agent"} onClick={() => setOpen(true)}>
          {selected ? <AgentGlyph id={selected.id} name={selected.name} size={24} /> : <PackageIcon className="w-5 h-5 flex-shrink-0" />}
          <span className="mux-agent-picker-trigger-name">{selected?.name ?? "选择 Agent"}</span>
          <ChevronDownIcon className="mux-agent-picker-chevron" />
        </button>
      </div>
    </div>
    {open && <AgentHandPicker agents={available} pinnedIds={pinnedIds} ready={pinned.ready} saving={pinned.saving}
      anchorRef={anchorRef} triggerRef={triggerRef} onSavePins={pinned.commit}
      onClose={() => setOpen(false)} onSelect={onSelectAgent}
      onAdd={onAddAgent ? () => { setOpen(false); onAddAgent(); } : undefined} />}
  </div>;
}
