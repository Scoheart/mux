import { useMemo, useState, type ReactNode } from "react";
import { DialogShell } from "./DialogShell";
import { SearchBar } from "./ui";

export interface ResourcePickerOption {
  id: string;
  name: string;
  description?: string;
  meta?: ReactNode;
  avatar?: ReactNode;
  disabled?: boolean;
}

export function ResourcePickerDialog({
  title,
  subtitle,
  options,
  addLabel = "添加",
  onAdd,
  onClose,
}: {
  title: string;
  subtitle?: ReactNode;
  options: ResourcePickerOption[];
  addLabel?: string;
  onAdd: (option: ResourcePickerOption) => Promise<unknown> | unknown;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const filtered = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    if (!needle) return options;
    return options.filter((option) =>
      `${option.name} ${option.description ?? ""}`.toLocaleLowerCase().includes(needle),
    );
  }, [options, query]);
  const selected = options.find((option) => option.id === selectedId) ?? null;

  const add = async () => {
    if (!selected || selected.disabled || busy) return;
    setBusy(true);
    try {
      await onAdd(selected);
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
      footerStart={<span className="mux-picker-count">{filtered.length} 个可选项</span>}
      footerEnd={
        <>
          <button type="button" className="btn-ghost" disabled={busy} onClick={onClose}>取消</button>
          <button type="button" className="btn-primary" disabled={!selected || selected.disabled || busy} onClick={() => void add()}>
            {busy ? "添加中…" : addLabel}
          </button>
        </>
      }
    >
      <div className="mux-picker-search">
        <SearchBar value={query} onChange={setQuery} placeholder="搜索资源" autoFocus />
      </div>
      <div className="mux-picker-list" role="listbox" aria-label={title}>
        {filtered.length === 0 ? (
          <div className="mux-picker-empty">没有匹配项</div>
        ) : filtered.map((option) => (
          <button
            key={option.id}
            type="button"
            role="option"
            aria-selected={selectedId === option.id}
            className="mux-picker-option"
            data-selected={selectedId === option.id ? "true" : undefined}
            disabled={option.disabled}
            onClick={() => setSelectedId(option.id)}
          >
            {option.avatar}
            <span className="mux-picker-option-copy">
              <strong>{option.name}</strong>
              {option.description && <small>{option.description}</small>}
            </span>
            {option.meta && <span className="mux-picker-option-meta">{option.meta}</span>}
          </button>
        ))}
      </div>
    </DialogShell>
  );
}
