import { useEffect, useMemo, useRef, useState } from "react";
import type { AgentInfo } from "../lib/types";
import {
  ChevronDownIcon,
  CheckIcon,
  PackageIcon,
  PlusIcon,
  SearchIcon,
  XIcon,
} from "./icons";
import { AgentGlyph } from "./brandIcons";

interface AgentPickerProps {
  agents: AgentInfo[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onAddAgent?: () => void;
  /** Align the popup to the left edge of the trigger (default right). */
  menuAlign?: "left" | "right";
}

/** Searchable Agent select — used in the MCP toolbar under the feature tabs. */
export function AgentPicker({
  agents,
  selectedId,
  onSelect,
  onAddAgent,
  menuAlign = "left",
}: AgentPickerProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const anchorRef = useRef<HTMLDivElement>(null);

  const selected = selectedId ? agents.find((agent) => agent.id === selectedId) ?? null : null;
  const writableCount = agents.filter((agent) => agent.has_global).length;
  const visible = useMemo(() => {
    const q = query.trim().toLocaleLowerCase();
    return agents
      .filter((agent) => agent.has_global)
      .filter((agent) => {
        if (!q) return true;
        return [agent.name, agent.id, agent.category]
          .join(" ")
          .toLocaleLowerCase()
          .includes(q);
      })
      .sort((left, right) =>
        left.name.localeCompare(right.name, undefined, { sensitivity: "base" })
      );
  }, [agents, query]);

  useEffect(() => {
    if (!open) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    const closeOnPointerDown = (event: PointerEvent) => {
      if (!anchorRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("keydown", closeOnEscape);
    document.addEventListener("pointerdown", closeOnPointerDown);
    return () => {
      document.removeEventListener("keydown", closeOnEscape);
      document.removeEventListener("pointerdown", closeOnPointerDown);
    };
  }, [open]);

  return (
    <div className="mux-agent-picker-anchor flex-shrink-0" ref={anchorRef}>
      <button
        type="button"
        className="mux-agent-picker-trigger"
        data-active={selected ? "true" : undefined}
        data-open={open ? "true" : undefined}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => {
          setOpen((v) => !v);
          setQuery("");
        }}
      >
        {selected ? (
          <AgentGlyph id={selected.id} name={selected.name} size={24} />
        ) : (
          <PackageIcon className="w-5 h-5 flex-shrink-0" />
        )}
        <span className="mux-agent-picker-trigger-copy">
          <span className="mux-agent-picker-trigger-name">
            {selected?.name ?? "选择 Agent"}
          </span>
          <span className="mux-agent-picker-trigger-meta">
            {selected?.id ?? `${writableCount} 个可配置 Agent`}
          </span>
        </span>
        <ChevronDownIcon className="mux-agent-picker-chevron" />
      </button>
      {open && (
        <section
          className="mux-agent-picker"
          data-align={menuAlign}
          aria-label="选择 Agent"
        >
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

          <div className="mux-agent-picker-list" role="listbox">
            {visible.length === 0 ? (
              <div className="mux-agent-picker-empty">未找到匹配项</div>
            ) : (
              visible.map((agent) => {
                const active = selected?.id === agent.id;
                return (
                  <button
                    type="button"
                    role="option"
                    aria-selected={active}
                    key={agent.id}
                    className="mux-agent-picker-row"
                    data-active={active ? "true" : undefined}
                    onClick={() => {
                      onSelect(agent.id);
                      setOpen(false);
                    }}
                  >
                    <AgentGlyph id={agent.id} name={agent.name} size={32} />
                    <span className="min-w-0 flex-1">
                      <span className="mux-agent-picker-name">{agent.name}</span>
                      <span className="mux-agent-picker-meta">
                        {agent.format.toUpperCase()} · {agent.id}
                      </span>
                    </span>
                    {active && <CheckIcon className="mux-agent-picker-check" />}
                  </button>
                );
              })
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
  );
}
