import { useLayoutEffect, useRef, useState } from "react";
import type { AgentInfo } from "../lib/types";
import { MAX_PINNED_AGENTS, movePinnedAgentBy, previewPinnedAgentOrder } from "../lib/pinnedAgents";
import { AgentGlyph } from "./brandIcons";
import { PlusIcon } from "./icons";

export function PinnedAgentDock({ agents, ids, selectedId, expanded = false, disabled,
  replacementName, onReplace, onSelect, onEmpty, onReorder,
}: {
  agents: AgentInfo[];
  ids: string[];
  selectedId?: string | null;
  expanded?: boolean;
  disabled: boolean;
  replacementName?: string;
  onReplace?(index: number): void;
  onSelect(id: string): void;
  onEmpty?(): void;
  onReorder(ids: string[], movedId: string): void;
}) {
  const [preview, setPreview] = useState<string[] | null>(null);
  const dragged = useRef<string | null>(null);
  const previewRef = useRef<string[] | null>(null);
  const dock = useRef<HTMLElement>(null);
  const positions = useRef(new Map<string, number>());
  const order = preview ?? ids;
  // Fresh inventory arrays do not mean the pin order changed.
  const orderKey = order.join("\0");

  useLayoutEffect(() => {
    // AgentGlyph also exposes data-agent-id. Measure only the direct slot
    // buttons, otherwise each inner glyph overwrites its button's position.
    const buttons = Array.from(dock.current?.querySelectorAll<HTMLElement>(":scope > button[data-pin-slot][data-agent-id]") ?? []);
    const measured = buttons.map((button) => ({ button, id: button.dataset.agentId!, left: button.offsetLeft }));
    const motions: Animation[] = [];
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    for (const { button, id, left } of measured) {
      const before = positions.current.get(id);
      if (!reduced && before !== undefined && before !== left) {
        motions.push(button.animate([{ transform: `translateX(${before - left}px)` }, { transform: "none" }], {
          duration: 170, easing: "cubic-bezier(.2,.8,.2,1)",
        }));
      }
    }
    positions.current = new Map(measured.map(({ id, left }) => [id, left]));
    return () => motions.forEach((motion) => motion.cancel());
  }, [orderKey]);

  const clearDrag = () => { dragged.current = null; previewRef.current = null; setPreview(null); };
  return <nav ref={dock} className={`mux-pinned-agent-bar${expanded ? " mux-hand-dock" : ""}`}
    aria-label="置顶 Agent" data-replacing={replacementName ? "true" : undefined}
    onDragLeave={(event) => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setPreview(null); }}>
    {Array.from({ length: expanded ? MAX_PINNED_AGENTS : order.length }, (_, index) => {
      const id = order[index];
      const agent = agents.find((agent) => agent.id === id);
      const label = replacementName ? `用 ${replacementName} 替换 ${agent?.name ?? "空位"}` : agent?.name ?? "空置顶位";
      return <button key={id ?? `empty-${index}`} type="button" className="mux-pinned-agent"
        data-agent-id={id} data-pin-slot={index} data-empty={!agent || undefined}
        data-active={selectedId === id ? "true" : undefined}
        aria-current={selectedId === id ? "page" : undefined}
        aria-label={label} title={replacementName ? label : agent ? `${agent.name} · 拖动排序，Option + 左右方向键调整` : label}
        aria-keyshortcuts="Alt+ArrowLeft Alt+ArrowRight" disabled={disabled}
        draggable={Boolean(agent) && !disabled && !replacementName}
        onClick={() => replacementName ? onReplace?.(index) : agent ? onSelect(id) : onEmpty?.()}
        onKeyDown={(event) => {
          if (!agent || disabled || replacementName || !event.altKey || !["ArrowLeft", "ArrowRight"].includes(event.key)) return;
          event.preventDefault();
          onReorder(movePinnedAgentBy(ids, id, event.key === "ArrowLeft" ? -1 : 1), id);
        }}
        onDragStart={(event) => {
          if (!agent || disabled || replacementName) { event.preventDefault(); return; }
          dragged.current = id; previewRef.current = ids;
          event.dataTransfer.effectAllowed = "move";
          event.dataTransfer.setData("text/plain", id);
        }}
        onDragOver={(event) => {
          if (!dragged.current || disabled || replacementName) return;
          event.preventDefault(); event.dataTransfer.dropEffect = "move";
          const bounds = event.currentTarget.getBoundingClientRect();
          const next = agent ? previewPinnedAgentOrder(ids, dragged.current, id,
            event.clientX >= bounds.left + bounds.width / 2 ? "after" : "before")
            : [...ids.filter((id) => id !== dragged.current), dragged.current];
          previewRef.current = next;
          setPreview(next);
        }}
        onDrop={(event) => {
          if (!dragged.current || disabled || replacementName) return;
          event.preventDefault();
          onReorder(previewRef.current ?? ids, dragged.current);
          clearDrag();
        }} onDragEnd={clearDrag}>
        {agent ? <span className="mux-pinned-agent-glyph"><AgentGlyph id={agent.id} name={agent.name} size={30} /></span>
          : <PlusIcon className="w-3.5 h-3.5" />}
      </button>;
    })}
  </nav>;
}
