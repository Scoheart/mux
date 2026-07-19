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
  actionLabel,
  busyLabel,
  emptyMessage = "没有可选资产",
  searchPlaceholder = "搜索资产",
  onSelect,
  onClose,
}: {
  title: string;
  subtitle: ReactNode;
  mode: "single" | "multiple";
  options: ConsumptionPickerOption[];
  actionLabel: string;
  busyLabel?: string;
  emptyMessage?: string;
  searchPlaceholder?: string;
  onSelect(ids: string[]): Promise<unknown> | unknown;
  onClose(): void;
}) {
  const [query, setQuery] = useState("");
  const [selectedIds, setSelectedIds] = useState(() => new Set<string>());
  const [busy, setBusy] = useState(false);
  const filtered = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    return options.filter((option) =>
      !needle || `${option.name} ${option.description ?? ""}`.toLocaleLowerCase().includes(needle),
    );
  }, [options, query]);

  const toggle = (id: string) => {
    setSelectedIds((current) => {
      if (mode === "single") return new Set([id]);
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const apply = async () => {
    if (selectedIds.size === 0) return;
    setBusy(true);
    try {
      await onSelect([...selectedIds].sort());
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
      footerStart={
        <span className="mux-picker-count">
          {selectedIds.size > 0 ? `已选 ${selectedIds.size} · ` : ""}{filtered.length} 项
        </span>
      }
      footerEnd={
        <>
          <button type="button" className="btn-ghost" disabled={busy} onClick={onClose}>取消</button>
          <button type="button" className="btn-primary" disabled={selectedIds.size === 0 || busy} onClick={() => void apply()}>
            {busy
              ? (busyLabel ?? `${actionLabel}中…`)
              : mode === "multiple" && selectedIds.size > 1
                ? `${actionLabel}（${selectedIds.size}）`
                : actionLabel}
          </button>
        </>
      }
    >
      <div className="mux-picker-search">
        <SearchBar value={query} onChange={setQuery} placeholder={searchPlaceholder} autoFocus />
      </div>
      <div className="mux-picker-list" role={mode === "single" ? "listbox" : "group"} aria-label={title}>
        {filtered.length === 0 ? (
          <div className="mux-picker-empty">{emptyMessage}</div>
        ) : filtered.map((option) => (
          <button
            key={option.id}
            type="button"
            role={mode === "single" ? "option" : undefined}
            className="mux-picker-option mux-consumption-picker-option"
            data-selected={selectedIds.has(option.id) ? "true" : undefined}
            disabled={option.disabled || busy}
            aria-selected={mode === "single" ? selectedIds.has(option.id) : undefined}
            aria-pressed={mode === "multiple" ? selectedIds.has(option.id) : undefined}
            onClick={() => toggle(option.id)}
          >
            <span className="mux-consumption-picker-check" aria-hidden="true">
              {selectedIds.has(option.id) ? "✓" : ""}
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
