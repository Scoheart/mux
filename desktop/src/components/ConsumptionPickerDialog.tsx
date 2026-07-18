import { useMemo, useState, type ReactNode } from "react";
import { DialogShell } from "./DialogShell";
import { SearchBar } from "./ui";

export interface ConsumptionPickerOption {
  id: string;
  name: string;
  description?: string;
  meta?: ReactNode;
  disabled?: boolean;
  reason?: string;
}

export function ConsumptionPickerDialog({
  title,
  subtitle,
  mode,
  options,
  selectedIds,
  onReview,
  onClose,
}: {
  title: string;
  subtitle: ReactNode;
  mode: "single" | "multiple";
  options: ConsumptionPickerOption[];
  selectedIds: string[];
  onReview(ids: string[]): Promise<unknown> | unknown;
  onClose(): void;
}) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(() => new Set(selectedIds));
  const [busy, setBusy] = useState(false);
  const filtered = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    return options.filter((option) =>
      !needle || `${option.name} ${option.description ?? ""}`.toLocaleLowerCase().includes(needle),
    );
  }, [options, query]);

  const toggle = (id: string) => {
    setSelected((current) => {
      if (mode === "single") return current.has(id) ? new Set() : new Set([id]);
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const review = async () => {
    setBusy(true);
    try {
      await onReview([...selected].sort());
    } finally {
      setBusy(false);
    }
  };

  return (
    <DialogShell
      kind="picker"
      title={title}
      subtitle={subtitle}
      busy={busy}
      onClose={onClose}
      footerStart={<span className="mux-picker-count">已选择 {selected.size} 项</span>}
      footerEnd={
        <>
          <button type="button" className="btn-ghost" disabled={busy} onClick={onClose}>取消</button>
          <button type="button" className="btn-primary" disabled={busy} onClick={() => void review()}>
            {busy ? "生成计划…" : "审阅变更"}
          </button>
        </>
      }
    >
      <div className="mux-picker-search">
        <SearchBar value={query} onChange={setQuery} placeholder="搜索中央资产" autoFocus />
      </div>
      <div className="mux-picker-list" role={mode === "single" ? "radiogroup" : "group"} aria-label={title}>
        {filtered.length === 0 ? (
          <div className="mux-picker-empty">没有匹配的中央资产</div>
        ) : filtered.map((option) => (
          <button
            key={option.id}
            type="button"
            className="mux-picker-option mux-consumption-picker-option"
            data-selected={selected.has(option.id) ? "true" : undefined}
            disabled={option.disabled || busy}
            aria-pressed={selected.has(option.id)}
            onClick={() => toggle(option.id)}
          >
            <span className="mux-consumption-picker-check" aria-hidden="true">
              {selected.has(option.id) ? "✓" : ""}
            </span>
            <span className="mux-picker-option-copy">
              <strong>{option.name}</strong>
              <small>{option.description}</small>
              {option.reason && <em>{option.reason}</em>}
            </span>
            {option.meta && <span className="mux-picker-option-meta">{option.meta}</span>}
          </button>
        ))}
      </div>
    </DialogShell>
  );
}
