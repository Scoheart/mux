import { useEffect, useMemo, useRef, useState } from "react";
import type { DragEvent, KeyboardEvent } from "react";
import type { AgentInfo } from "../lib/types";
import {
  buildAgentPickerSections,
  MAX_PINNED_AGENTS,
  movePinnedAgentAfter,
  movePinnedAgentBefore,
  movePinnedAgentBy,
  togglePinnedAgent,
} from "../lib/pinnedAgents";
import { usePinnedAgents } from "../hooks/usePinnedAgents";
import { AgentGlyph } from "./brandIcons";
import {
  CheckIcon,
  ChevronDownIcon,
  GripVerticalIcon,
  PackageIcon,
  PinIcon,
  PlusIcon,
  SearchIcon,
  XIcon,
} from "./icons";
import {
  claimLayerKeyboardEvent,
  MODAL_DIALOG_SELECTOR,
  wasHandledByLayer,
} from "./ui";

const PIN_LIMIT_DESCRIPTION_ID = "mux-agent-pin-limit-description";

interface AgentNavigationProps {
  agents: AgentInfo[];
  selectedAgentId: string | null;
  onSelectAgent(id: string): void;
  onAddAgent?: () => void;
}

export function AgentNavigation({
  agents,
  selectedAgentId,
  onSelectAgent,
  onAddAgent,
}: AgentNavigationProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [draggedId, setDraggedId] = useState<string | null>(null);
  const [announcement, setAnnouncement] = useState("");
  const anchorRef = useRef<HTMLDivElement>(null);
  const { agentIds, ready, saving, commit } = usePinnedAgents();
  const sections = useMemo(
    () => buildAgentPickerSections(agents, agentIds, query),
    [agentIds, agents, query],
  );
  const pinnedIds = sections.pinned.map(({ id }) => id);
  const selectedAgent = agents.find(({ id }) => id === selectedAgentId) ?? null;
  const pinLimitReached = pinnedIds.length >= MAX_PINNED_AGENTS;

  useEffect(() => {
    if (!open) return;
    const closeOnEscape = (event: globalThis.KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (wasHandledByLayer(event) || document.querySelector(MODAL_DIALOG_SELECTOR)) return;
      claimLayerKeyboardEvent(event);
      setOpen(false);
    };
    const closeOnPointerDown = (event: PointerEvent) => {
      if (document.querySelector(MODAL_DIALOG_SELECTOR)) return;
      if (!anchorRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("keydown", closeOnEscape, true);
    document.addEventListener("pointerdown", closeOnPointerDown);
    return () => {
      document.removeEventListener("keydown", closeOnEscape, true);
      document.removeEventListener("pointerdown", closeOnPointerDown);
    };
  }, [open]);

  const selectAgent = (id: string) => {
    onSelectAgent(id);
    setOpen(false);
  };

  const sameOrder = (left: string[], right: string[]) =>
    left.join("\u0000") === right.join("\u0000");

  const saveOrder = (next: string[], movedId: string) => {
    if (!ready || saving || sameOrder(next, pinnedIds)) return;
    void commit(next).then((saved) => {
      if (!saved) return;
      const moved = agents.find(({ id }) => id === movedId);
      setAnnouncement(`${moved?.name ?? movedId} 已移动到第 ${next.indexOf(movedId) + 1} 位`);
    });
  };

  const togglePin = (id: string) => {
    if (!ready || saving) return;
    const wasPinned = pinnedIds.includes(id);
    if (!wasPinned && pinLimitReached) return;
    const next = togglePinnedAgent(pinnedIds, id);
    if (sameOrder(next, pinnedIds)) return;
    void commit(next).then((saved) => {
      if (!saved) return;
      const changed = agents.find((agent) => agent.id === id);
      setAnnouncement(`${changed?.name ?? id} 已${wasPinned ? "取消置顶" : "置顶"}`);
    });
  };

  const moveByKeyboard = (event: KeyboardEvent<HTMLButtonElement>, id: string) => {
    if (
      !ready ||
      saving ||
      !event.altKey ||
      (event.key !== "ArrowUp" && event.key !== "ArrowDown")
    ) return;
    event.preventDefault();
    const next = movePinnedAgentBy(pinnedIds, id, event.key === "ArrowUp" ? -1 : 1);
    saveOrder(next, id);
  };

  const dropAtRow = (event: DragEvent<HTMLDivElement>, targetId: string) => {
    event.preventDefault();
    if (!ready || saving || !draggedId) return;
    const bounds = event.currentTarget.getBoundingClientRect();
    const afterTarget = event.clientY >= bounds.top + bounds.height / 2;
    const next = afterTarget
      ? movePinnedAgentAfter(pinnedIds, draggedId, targetId)
      : movePinnedAgentBefore(pinnedIds, draggedId, targetId);
    const movedId = draggedId;
    setDraggedId(null);
    saveOrder(next, movedId);
  };

  const agentRow = (agent: AgentInfo, isPinned: boolean, sortable: boolean) => {
    const active = selectedAgentId === agent.id;
    const mutationUnavailable = !ready || saving;
    const pinLimitBlocked = !isPinned && pinLimitReached;
    return (
      <div
        key={agent.id}
        className="mux-agent-picker-row"
        data-active={active ? "true" : undefined}
        data-dragging={draggedId === agent.id ? "true" : undefined}
        onDragOver={sortable ? (event) => event.preventDefault() : undefined}
        onDrop={sortable ? (event) => dropAtRow(event, agent.id) : undefined}
      >
        {sortable && (
          <button
            type="button"
            className="mux-agent-order-handle"
            draggable={ready && !saving}
            disabled={mutationUnavailable}
            title="拖拽排序；Option + 上下方向键调整"
            aria-label={`调整 ${agent.name} 的置顶顺序`}
            onDragStart={(event) => {
              event.dataTransfer.effectAllowed = "move";
              setDraggedId(agent.id);
            }}
            onDragEnd={() => setDraggedId(null)}
            onKeyDown={(event) => moveByKeyboard(event, agent.id)}
          >
            <GripVerticalIcon className="w-4 h-4" />
          </button>
        )}
        <button
          type="button"
          className="mux-agent-picker-select"
          aria-current={active ? "page" : undefined}
          onClick={() => selectAgent(agent.id)}
        >
          <AgentGlyph id={agent.id} name={agent.name} size={32} />
          <span className="min-w-0 flex-1">
            <span className="mux-agent-picker-name">{agent.name}</span>
            <span className="mux-agent-picker-meta">{agent.format.toUpperCase()} · {agent.id}</span>
          </span>
          {active && <CheckIcon className="mux-agent-picker-check" />}
        </button>
        <button
          type="button"
          className="mux-agent-pin-action"
          data-pinned={isPinned ? "true" : undefined}
          disabled={mutationUnavailable}
          aria-disabled={pinLimitBlocked || undefined}
          aria-describedby={pinLimitBlocked ? PIN_LIMIT_DESCRIPTION_ID : undefined}
          title={isPinned ? "取消置顶" : "置顶"}
          aria-label={`${isPinned ? "取消置顶" : "置顶"} ${agent.name}`}
          aria-pressed={isPinned}
          onClick={(event) => {
            if (pinLimitBlocked) {
              event.preventDefault();
              return;
            }
            togglePin(agent.id);
          }}
        >
          {isPinned ? <XIcon className="w-3.5 h-3.5" /> : <PinIcon className="w-3.5 h-3.5" />}
        </button>
      </div>
    );
  };

  return (
    <div className="mux-agent-navigation">
      <span className="sr-only" aria-live="polite">{announcement}</span>
      <span id={PIN_LIMIT_DESCRIPTION_ID} className="sr-only">
        最多可置顶六个 Agent，请先取消一个置顶后再添加。
      </span>
      {sections.pinned.length > 0 && (
        <nav className="mux-pinned-agent-bar" aria-label="置顶 Agent">
          {sections.pinned.map((agent) => (
            <button
              type="button"
              key={agent.id}
              className="mux-pinned-agent"
              data-active={selectedAgentId === agent.id ? "true" : undefined}
              aria-current={selectedAgentId === agent.id ? "page" : undefined}
              aria-label={agent.name}
              title={agent.name}
              onClick={() => onSelectAgent(agent.id)}
            >
              <span className="mux-pinned-agent-glyph">
                <AgentGlyph id={agent.id} name={agent.name} size={30} />
              </span>
            </button>
          ))}
        </nav>
      )}

      <div className="mux-agent-picker-anchor" ref={anchorRef}>
        <button
          type="button"
          className="mux-agent-picker-trigger"
          data-active={selectedAgent ? "true" : undefined}
          data-open={open ? "true" : undefined}
          aria-haspopup="dialog"
          aria-expanded={open}
          title={selectedAgent?.name}
          onClick={() => {
            setOpen((wasOpen) => {
              if (!wasOpen) setQuery("");
              return !wasOpen;
            });
          }}
        >
          {selectedAgent ? (
            <AgentGlyph id={selectedAgent.id} name={selectedAgent.name} size={24} />
          ) : (
            <PackageIcon className="w-5 h-5 flex-shrink-0" />
          )}
          <span className="mux-agent-picker-trigger-name">
            {selectedAgent?.name ?? "选择 Agent"}
          </span>
          <ChevronDownIcon className="mux-agent-picker-chevron" />
        </button>

        {open && (
          <section className="mux-agent-picker" role="dialog" aria-label="选择和置顶 Agent">
            <div className="mux-agent-picker-search">
              <SearchIcon className="w-4 h-4 flex-shrink-0" />
              <input
                type="search"
                autoFocus
                spellCheck={false}
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="按名称或 ID 搜索"
                aria-label="搜索 Agent"
              />
              <button
                type="button"
                className="mux-agent-picker-search-clear"
                data-visible={query ? "true" : undefined}
                disabled={!query}
                tabIndex={query ? 0 : -1}
                aria-label="清除搜索"
                title="清除搜索"
                onPointerDown={(event) => event.preventDefault()}
                onClick={() => setQuery("")}
              >
                <XIcon className="w-3.5 h-3.5" />
              </button>
            </div>

            <div className="mux-agent-picker-list">
              {sections.searchResults ? (
                sections.searchResults.length > 0 ? (
                  sections.searchResults.map((agent) =>
                    agentRow(agent, pinnedIds.includes(agent.id), false),
                  )
                ) : (
                  <div className="mux-agent-picker-empty">未找到匹配项</div>
                )
              ) : (
                <>
                  <div className="mux-agent-picker-section-heading">
                    <span>已置顶</span><span>{sections.pinned.length}/{MAX_PINNED_AGENTS}</span>
                  </div>
                  {sections.pinned.length > 0 ? (
                    sections.pinned.map((agent) => agentRow(agent, true, true))
                  ) : (
                    <div className="mux-agent-picker-hint">在常用 Agent 右侧点击 Pin</div>
                  )}
                  <div className="mux-agent-picker-section-heading"><span>全部 Agent</span></div>
                  {sections.available.map((agent) => agentRow(agent, false, false))}
                </>
              )}
            </div>

            {onAddAgent && (
              <div className="mux-agent-picker-footer">
                <button
                  type="button"
                  onClick={() => {
                    setOpen(false);
                    onAddAgent();
                  }}
                >
                  <PlusIcon className="w-4 h-4" />
                  添加自定义 Agent
                </button>
              </div>
            )}
          </section>
        )}
      </div>
    </div>
  );
}
