import { startTransition, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type RefObject } from "react";
import type { AgentInfo } from "../lib/types";
import { MAX_PINNED_AGENTS } from "../lib/pinnedAgents";
import { agentHandMetrics } from "../lib/agentHandLayout";
import { AgentGlyph } from "./brandIcons";
import { ArrowLeftIcon, PinIcon, PinOffIcon, PlusIcon, SearchIcon } from "./icons";
import { Modal } from "./ui";
import { PinnedAgentDock } from "./PinnedAgentDock";
import { AgentEntryTransition, type AgentEntryOrigin } from "./AgentEntryTransition";
import "./AgentHandPicker.css";

type Group = "pinned" | "builtin" | "custom";
type HandItem = { id: string; agent?: AgentInfo };
const ADD_ID = "__mux_add_agent__";
const reducedMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const pose = (x: number, y: number, angle = 0, scale = 1) => `translate3d(${x}px,${y}px,0) rotate(${angle}deg) scale(${scale})`;

export function AgentHandPicker({ agents, pinnedIds, ready, saving, anchorRef, triggerRef, onSavePins, onSelect, onAdd, onClose }: {
  agents: AgentInfo[];
  pinnedIds: string[];
  ready: boolean;
  saving: boolean;
  anchorRef: RefObject<HTMLDivElement | null>;
  triggerRef: RefObject<HTMLButtonElement | null>;
  onSavePins(ids: string[]): Promise<boolean>;
  onSelect(id: string): void;
  onAdd?: () => void;
  onClose(): void;
}) {
  const [group, setGroup] = useState<Group>("builtin");
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(0);
  const [width, setWidth] = useState(Math.min(window.innerWidth * .6 + 96, window.innerWidth - 144));
  const [viewportWidth, setViewportWidth] = useState(window.innerWidth);
  const [dockPosition, setDockPosition] = useState({ left: 16, top: 16 });
  const [highlighted, setHighlighted] = useState<string | null>(null);
  const [replacement, setReplacement] = useState<AgentInfo | null>(null);
  const [announcement, setAnnouncement] = useState("");
  const [busy, setBusy] = useState(false);
  const [playing, setPlaying] = useState<string | null>(null);
  const [entryTransition, setEntryTransition] = useState<{ agent: AgentInfo; origin: AgentEntryOrigin; overlay: HTMLElement | null } | null>(null);
  const boardRef = useRef<HTMLDivElement>(null);
  const dockRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const cardRefs = useRef(new Map<string, HTMLElement>());
  const motions = useRef(new Set<Animation>());
  const lock = useRef(false);
  const mounted = useRef(true);
  const entered = useRef(false);
  const keyboard = useRef(false);
  const focusFrame = useRef<number | null>(null);

  const rows = useMemo(() => {
    const text = query.trim().toLocaleLowerCase();
    if (text) return agents.filter((a) => [a.name, a.id, a.category].join(" ").toLocaleLowerCase().includes(text));
    if (group === "pinned") return pinnedIds.flatMap((id) => agents.find((a) => a.id === id) ?? []);
    return agents.filter((a) => group === "builtin" ? a.builtin : !a.builtin);
  }, [agents, pinnedIds, group, query]);
  const items = useMemo<HandItem[]>(() => [
    ...rows.map((agent) => ({ id: agent.id, agent })),
    ...(!query.trim() && group === "custom" && onAdd ? [{ id: ADD_ID }] : []),
  ], [rows, group, query, onAdd]);
  const pageSize = width < 430 ? 3 : width < 620 ? 6 : width < 780 ? 9 : 10;
  const pageCount = Math.max(1, Math.ceil(items.length / pageSize));
  const actualPage = Math.min(page, pageCount - 1);
  const visible = items.slice(actualPage * pageSize, (actualPage + 1) * pageSize);
  const handSpan = Math.min(viewportWidth * .6, width - 32);
  const cardWidth = width < 430 ? 76 : Math.max(84, Math.min(112, handSpan / 8.5));
  const cardHeight = cardWidth * 4 / 3;
  const metrics = agentHandMetrics(visible.length, cardWidth);
  // Match the visible fan's outer edges, including the rotated end cards.
  const angle = metrics.spread * Math.PI / 180;
  const edgeWidth = cardWidth * Math.cos(angle) + cardHeight * Math.sin(angle);
  const desiredStep = (handSpan - edgeWidth) / Math.max(1, visible.length - 1);
  const step = visible.length >= 5 ? Math.max(0, desiredStep) : Math.min(metrics.step * 1.2, Math.max(0, desiredStep));
  const positions = visible.map((_, index) => {
    const unit = visible.length <= 1 ? 0 : (index - (visible.length - 1) / 2) / ((visible.length - 1) / 2);
    return {
      x: width / 2 - cardWidth / 2 + (index - (visible.length - 1) / 2) * step,
      y: 128 - cardHeight / 2 + unit * unit * metrics.rise - metrics.rise / 2,
      angle: unit * metrics.spread,
    };
  });
  const handKey = `${visible.map(({ id }) => id).join("\0")}:${width}:${viewportWidth}`;

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false; entered.current = false;
      if (focusFrame.current !== null) cancelAnimationFrame(focusFrame.current);
      motions.current.forEach((motion) => motion.cancel()); motions.current.clear();
    };
  }, []);

  useEffect(() => {
    // Warm only the view module while the user chooses; no Agent data or writes.
    void import("./AgentView").catch(() => undefined);
  }, []);

  useLayoutEffect(() => {
    let frame: number | null = null;
    const measure = () => {
      if (boardRef.current) setWidth(boardRef.current.clientWidth);
      setViewportWidth(window.innerWidth);
      const bounds = anchorRef.current?.getBoundingClientRect();
      const topbarWidth = Math.min(590, window.innerWidth - 48);
      const next = { left: Math.max(24, Math.min(bounds?.left ?? (window.innerWidth - topbarWidth) / 2, window.innerWidth - topbarWidth - 24)),
        top: Math.max(8, bounds?.top ?? 16) };
      setDockPosition((previous) => previous.left === next.left && previous.top === next.top ? previous : next);
    };
    const scheduleMeasure = () => {
      if (frame !== null) return;
      frame = requestAnimationFrame(() => { frame = null; measure(); });
    };
    measure();
    const observer = new ResizeObserver(scheduleMeasure);
    if (boardRef.current) observer.observe(boardRef.current);
    window.addEventListener("resize", scheduleMeasure);
    return () => {
      if (frame !== null) cancelAnimationFrame(frame);
      observer.disconnect(); window.removeEventListener("resize", scheduleMeasure);
    };
  }, [anchorRef]);

  const animate = (element: HTMLElement, frames: Keyframe[], duration: number, delay = 0) => {
    if (reducedMotion()) return Promise.resolve();
    const motion = element.animate(frames, { duration, delay, easing: "cubic-bezier(.22,.7,.25,1)", fill: "both" });
    motions.current.add(motion);
    return motion.finished.catch(() => undefined).then(() => { motions.current.delete(motion); });
  };

  useLayoutEffect(() => {
    if (lock.current) return;
    setHighlighted(null);
    const board = boardRef.current;
    if (!board) return;
    const bounds = board.getBoundingClientRect();
    const source = triggerRef.current?.getBoundingClientRect();
    const first = !entered.current;
    const pile = pose(first && source ? source.left + source.width / 2 - bounds.left - cardWidth / 2 : width / 2 - cardWidth / 2,
      first && source ? source.top + source.height / 2 - bounds.top - cardHeight / 2 : 115, -10, .4);
    const deals: Animation[] = [];
    visible.forEach(({ id }, index) => {
      const card = cardRefs.current.get(id);
      if (!card || reducedMotion()) return;
      card.dataset.dealing = "true";
      const motion = card.animate([{ transform: pile, opacity: 0 }, { transform: card.style.transform, opacity: 1 }], {
        duration: 250, delay: index * Math.min(28, 168 / Math.max(1, visible.length - 1)), easing: "cubic-bezier(.2,.8,.2,1)", fill: "backwards",
      });
      deals.push(motion); motions.current.add(motion);
      motion.onfinish = () => { delete card.dataset.dealing; motions.current.delete(motion); };
    });
    entered.current = true;
    return () => {
      deals.forEach((motion) => { motion.cancel(); motions.current.delete(motion); });
      cardRefs.current.forEach((card) => { delete card.dataset.dealing; });
    };
    // The hand redeals only when its identities or measured layout change.
    // Pin/save/highlight renders must not replay the entrance animation.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [handKey]);

  const begin = () => {
    if (lock.current || saving) return false;
    lock.current = true; setBusy(true); setHighlighted(null); return true;
  };
  const finish = () => { lock.current = false; if (mounted.current) { setBusy(false); setPlaying(null); } };
  const freezeCards = () => {
    const current = [...cardRefs.current.values()].map((card) => ({ card, transform: getComputedStyle(card).transform, opacity: getComputedStyle(card).opacity }));
    motions.current.forEach((motion) => motion.cancel()); motions.current.clear();
    current.forEach(({ card, transform, opacity }) => { card.style.transform = transform; card.style.opacity = opacity; });
    return current;
  };
  const close = async () => {
    if (replacement && !lock.current) { setReplacement(null); searchRef.current?.focus(); setAnnouncement(""); return; }
    if (!begin()) return;
    const board = boardRef.current?.getBoundingClientRect();
    const source = triggerRef.current?.getBoundingClientRect();
    const target = pose(source && board ? source.left + source.width / 2 - board.left - cardWidth / 2 : width / 2 - cardWidth / 2,
      source && board ? source.top + source.height / 2 - board.top - cardHeight / 2 : 120, -10, .35);
    const cards = freezeCards();
    await Promise.all(cards.map(({ card, transform, opacity }, index) => animate(card, [{ transform, opacity }, { transform: target, opacity: 0 }], 150, (cards.length - index - 1) * Math.min(12, 72 / Math.max(1, cards.length - 1)))));
    if (mounted.current) onClose();
  };
  const select = (id: string) => {
    if (replacement || !begin()) return;
    const cards = freezeCards();
    const current = cards.find(({ card }) => card.dataset.handId === id);
    const agent = agents.find((agent) => agent.id === id);
    const tile = current?.card.querySelector<HTMLElement>(".mux-agent-hand-tile");
    if (!current || !tile || !agent || reducedMotion()) {
      onSelect(id); onClose(); return;
    }
    const bounds = tile.getBoundingClientRect();
    const matrix = new DOMMatrixReadOnly(current.transform === "none" ? undefined : current.transform);
    setPlaying(id);
    setEntryTransition({ agent, origin: {
      left: bounds.left + (bounds.width - tile.offsetWidth) / 2,
      top: bounds.top + (bounds.height - tile.offsetHeight) / 2,
      width: tile.offsetWidth, height: tile.offsetHeight,
      rotation: Math.atan2(matrix.b, matrix.a) * 180 / Math.PI,
    }, overlay: current.card.closest<HTMLElement>("[data-modal-overlay]") });
    cards.filter(({ card }) => card !== current.card).forEach(({ card, transform, opacity }, index) => {
      void animate(card, [{ transform, opacity }, { transform: pose(width / 2 - cardWidth / 2, 120, -8, .6), opacity: 0 }], 140, index * 8);
    });
    // Start loading now, but retain the revealed page if the lazy view suspends
    // instead of briefly replacing the whole content with a loading fallback.
    startTransition(() => onSelect(id));
  };
  const add = async () => {
    if (!onAdd || !begin()) return;
    freezeCards(); setPlaying(ADD_ID);
    const card = cardRefs.current.get(ADD_ID);
    if (card) {
      await animate(card, [{ transform: card.style.transform, opacity: 1 },
        { transform: pose(width / 2 - cardWidth / 2, 40, 0, 1.7), opacity: 0 }], 180);
    }
    if (mounted.current) onAdd();
  };
  const savePins = async (next: string[], agent: AgentInfo, slot?: number) => {
    const restoreKeyboardFocus = keyboard.current;
    if (!ready || !begin()) return;
    // Keep pin feedback immediate; persistence runs alongside the short flight.
    const result = onSavePins(next);
    const card = cardRefs.current.get(agent.id);
    const target = slot !== undefined ? dockRef.current?.querySelector<HTMLElement>(`[data-pin-slot="${slot}"]`) : null;
    let flight = Promise.resolve();
    if (card && target && boardRef.current) {
      const bounds = boardRef.current.getBoundingClientRect(), destination = target.getBoundingClientRect();
      setPlaying(agent.id);
      flight = animate(card, [{ transform: getComputedStyle(card).transform, opacity: 1 }, {
        transform: pose(destination.left + destination.width / 2 - bounds.left - cardWidth / 2,
          destination.top + destination.height / 2 - bounds.top - cardHeight / 2,
          0, Math.min(destination.width / cardWidth, destination.height / cardHeight)), opacity: .3,
      }], 280);
    }
    const [saved] = await Promise.all([result, flight]);
    if (!mounted.current) return;
    // Remove filled pin-flight styles without restarting every other card.
    card?.getAnimations().forEach((motion) => motion.cancel());
    finish(); setReplacement(null);
    setAnnouncement(saved ? `${agent.name} 已${next.includes(agent.id) ? "置顶" : "取消置顶"}` : "未保存，已恢复原来的置顶状态");
    // Pin changes can remove a card from the 常用 group. Restore focus deliberately.
    if (restoreKeyboardFocus || !card?.isConnected) {
      focusFrame.current = requestAnimationFrame(() => {
        focusFrame.current = null;
        if (!mounted.current) return;
        (card?.isConnected ? card.querySelector<HTMLButtonElement>(".mux-agent-hand-pin") : searchRef.current)?.focus();
      });
    }
  };
  const togglePin = (agent: AgentInfo) => {
    if (!ready || saving || lock.current) return;
    if (pinnedIds.includes(agent.id)) { void savePins(pinnedIds.filter((id) => id !== agent.id), agent); return; }
    if (pinnedIds.length >= MAX_PINNED_AGENTS) {
      setReplacement(agent); setHighlighted(null);
      setAnnouncement(`选择上方一个牌槽，替换为 ${agent.name}`);
      dockRef.current?.querySelector<HTMLButtonElement>("button")?.focus();
      return;
    }
    void savePins([...pinnedIds, agent.id], agent, pinnedIds.length);
  };
  const changeGroup = (next: Group) => { if (busy || saving) return; setGroup(next); setQuery(""); setPage(0); setReplacement(null); setAnnouncement(""); setHighlighted(null); };

  return <Modal width="min(calc(60vw + 96px), calc(100vw - 144px))" maxHeight="calc(100dvh - 120px)" ariaLabel="选择和置顶 Agent" layer="agent-hand" onClose={() => void close()}>
    <div className="mux-agent-hand-picker" data-busy={busy || saving || undefined} data-playing={playing || undefined} data-entering={entryTransition ? "true" : undefined}>
      <div className="mux-agent-hand-topbar" style={dockPosition}>
      <div ref={dockRef} className="mux-agent-hand-pinned">
        <PinnedAgentDock expanded agents={agents} ids={pinnedIds} disabled={!ready || saving || busy}
          replacementName={replacement?.name} onReplace={(index) => {
            if (!replacement) return;
            const next = [...pinnedIds]; next[index] = replacement.id;
            void savePins(next, replacement, index);
          }} onSelect={(id) => void select(id)} onEmpty={() => searchRef.current?.focus()}
          onReorder={(next, id) => {
            if (next.join("\0") === pinnedIds.join("\0") || !ready || lock.current) return;
            const agent = agents.find((agent) => agent.id === id);
            if (agent) void savePins(next, agent);
          }} />
      </div>
      <div className="mux-agent-hand-toolbar">
        <label className="mux-agent-hand-search"><SearchIcon className="w-4 h-4" />
          <input ref={searchRef} data-modal-initial-focus type="search" autoComplete="off" spellCheck={false} placeholder="搜索名称或 ID" aria-label="搜索 Agent"
            value={query} disabled={busy || saving} onChange={(event) => { setQuery(event.target.value); setPage(0); setReplacement(null); setAnnouncement(""); }} />
        </label>
      </div>
      </div>
      <div className="mux-agent-hand-stage">
      <button type="button" className="mux-agent-hand-page mux-agent-hand-page-prev" aria-label="上一手 Agent" title="上一页"
        disabled={actualPage === 0 || busy || saving || Boolean(replacement)} onClick={() => { setPage(actualPage - 1); setHighlighted(null); }}><ArrowLeftIcon className="w-5 h-5" /></button>
      <div ref={boardRef} className="mux-agent-hand-board" style={{ height: Math.max(234, cardHeight + metrics.rise + 76) }} onPointerLeave={() => { if (!keyboard.current) setHighlighted(null); }}
        onPointerMove={(event) => {
          if (busy || replacement || (!event.movementX && !event.movementY)) return;
          keyboard.current = false;
          const bounds = event.currentTarget.getBoundingClientRect();
          const x = event.clientX - bounds.left, y = event.clientY - bounds.top;
          let nearest = -1, distance = Infinity;
          positions.forEach((point, index) => { const dx = Math.abs(x - point.x - cardWidth / 2); if (dx < distance) { nearest = index; distance = dx; } });
          const point = positions[nearest];
          setHighlighted(point && distance < cardWidth * .65 && y > point.y - 32 && y < point.y + cardHeight + 5 ? visible[nearest].id : null);
        }}>
        {visible.map(({ id, agent }, index) => <article key={id} ref={(node) => { if (node) cardRefs.current.set(id, node); else cardRefs.current.delete(id); }}
          data-hand-id={id} className="mux-agent-hand-card" data-highlighted={highlighted === id || undefined} data-playing={playing === id || undefined}
          data-new={!agent || undefined} style={{ width: cardWidth, height: cardHeight, transform: pose(positions[index].x, positions[index].y, positions[index].angle), "--hand-order": index } as CSSProperties}>
          <div className="mux-agent-hand-tile">
            <button type="button" className="mux-agent-hand-select" disabled={busy || Boolean(replacement)}
              title={agent?.name ?? "添加自定义 Agent"} aria-label={agent ? `进入 ${agent.name}` : "添加自定义 Agent"}
              onFocus={(event) => { if (event.currentTarget.matches(":focus-visible")) { keyboard.current = true; setHighlighted(id); } }}
              onBlur={() => { if (keyboard.current) setHighlighted(null); }}
              onKeyDown={(event) => {
                if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key) || event.altKey) return;
                event.preventDefault(); keyboard.current = true;
                const next = event.key === "Home" ? 0 : event.key === "End" ? visible.length - 1 : (index + (event.key === "ArrowLeft" ? -1 : 1) + visible.length) % visible.length;
                cardRefs.current.get(visible[next].id)?.querySelector<HTMLButtonElement>(".mux-agent-hand-select")?.focus();
              }}
              onClick={() => agent ? void select(id) : void add()}>
              {agent ? <AgentGlyph id={id} name={agent.name} size={30} /> : <PlusIcon className="w-7 h-7" />}
              <span><strong>{agent?.name ?? "添加 Agent"}</strong></span>
            </button>
            {agent && <button type="button" className="mux-agent-hand-pin" title={pinnedIds.includes(id) ? "取消置顶" : "置顶"}
              aria-label={`${pinnedIds.includes(id) ? "取消置顶" : "置顶"} ${agent.name}`} aria-pressed={pinnedIds.includes(id)} disabled={!ready || busy || saving || Boolean(replacement)}
              onFocus={(event) => { if (event.currentTarget.matches(":focus-visible")) { keyboard.current = true; setHighlighted(id); } }} onBlur={() => { if (keyboard.current) setHighlighted(null); }}
              onClick={() => togglePin(agent)}>{pinnedIds.includes(id) ? <PinOffIcon className="w-3.5 h-3.5" /> : <PinIcon className="w-3.5 h-3.5" />}</button>}
          </div>
        </article>)}
        {!visible.length && <p className="mux-agent-hand-empty">{query ? "未找到匹配的 Agent" : "点击卡片上的图钉，将常用 Agent 放到这里"}</p>}
      </div>
      <button type="button" className="mux-agent-hand-page mux-agent-hand-page-next" aria-label="下一手 Agent" title="下一页"
        disabled={actualPage + 1 === pageCount || busy || saving || Boolean(replacement)} onClick={() => { setPage(actualPage + 1); setHighlighted(null); }}><ArrowLeftIcon className="w-5 h-5 rotate-180" /></button>
      </div>
      <div className="mux-agent-hand-pagination">
        <span>{actualPage + 1} / {pageCount}</span>
        <span aria-hidden="true">·</span>
        <span>{rows.length} 个 Agent</span>
      </div>
      <div className="mux-agent-hand-footer">
      <div className="mux-agent-hand-filters" role="group" aria-label="Agent 分类">
        {([['pinned', '常用'], ['builtin', '内置'], ['custom', '自定义']] as const).map(([id, label]) => <button type="button" key={id}
          aria-pressed={group === id && !query.trim()} disabled={busy || saving} onClick={() => changeGroup(id)}>{label}</button>)}
      </div>
      <p className="mux-agent-hand-announcement" aria-live="polite">{replacement ? `选择上方牌槽，替换为 ${replacement.name} · Esc 取消` : announcement}</p>
      </div>
    </div>
    {entryTransition && <AgentEntryTransition {...entryTransition} onDone={onClose} />}
  </Modal>;
}
