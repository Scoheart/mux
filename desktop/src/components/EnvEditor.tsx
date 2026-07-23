import { useState, useEffect } from "react";
import { PlusIcon, TrashIcon } from "./icons";

let _uid = 0;
const nextId = () => ++_uid;

interface Row { id: number; k: string; v: string; }

interface EnvEditorProps {
  value: Record<string, string>;
  onChange: (env: Record<string, string>) => void;
}

function rowsToEnv(rows: Row[]): Record<string, string> {
  const env: Record<string, string> = {};
  rows.forEach((r) => {
    if (r.k.trim()) env[r.k.trim()] = r.v;
  });
  return env;
}

function envToRows(env: Record<string, string>): Row[] {
  const rows = Object.entries(env).map(([k, v]) => ({ id: nextId(), k, v }));
  return rows.length > 0 ? rows : [{ id: nextId(), k: "", v: "" }];
}

export function EnvEditor({ value, onChange }: EnvEditorProps) {
  const [rows, setRows] = useState<Row[]>(() => envToRows(value));

  // Resync only when the external value genuinely diverges from what we already
  // project (an external reset) — never clobber in-progress edits or churn on a
  // fresh `{}` reference from the parent.
  useEffect(() => {
    if (JSON.stringify(rowsToEnv(rows)) !== JSON.stringify(value)) {
      setRows(envToRows(value));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value]);

  const updateRows = (next: Row[]) => {
    const clean = next.length > 0 ? next : [{ id: nextId(), k: "", v: "" }];
    setRows(clean);
    onChange(rowsToEnv(clean));
  };

  const setKey = (id: number, k: string) =>
    updateRows(rows.map((r) => (r.id === id ? { ...r, k } : r)));

  const setVal = (id: number, v: string) =>
    updateRows(rows.map((r) => (r.id === id ? { ...r, v } : r)));

  const removeRow = (id: number) =>
    updateRows(rows.filter((r) => r.id !== id));

  const addRow = () => updateRows([...rows, { id: nextId(), k: "", v: "" }]);

  const inputStyle = {
    background: "var(--surface-app)",
    border: "1px solid var(--border-hairline)",
    color: "var(--text-primary)",
    fontFamily: "var(--font-mono)",
    fontSize: 11,
    padding: "4px 8px",
    borderRadius: 6,
    outline: "none",
    width: "100%",
  } as const;

  return (
    <div className="mt-2 space-y-1.5">
      {rows.map((row) => (
        <div key={row.id} className="flex items-center gap-1.5">
          <input
            style={{ ...inputStyle, flex: "0 0 38%" }}
            placeholder="KEY"
            value={row.k}
            onChange={(e) => setKey(row.id, e.target.value)}
          />
          <span style={{ color: "var(--text-secondary)", fontSize: 11, flexShrink: 0 }}>=</span>
          <input
            style={{ ...inputStyle, flex: 1 }}
            placeholder="value"
            value={row.v}
            onChange={(e) => setVal(row.id, e.target.value)}
          />
          <button
            onClick={() => removeRow(row.id)}
            className="flex-shrink-0 w-5 h-5 flex items-center justify-center rounded opacity-50 hover:opacity-100 border-0 bg-transparent cursor-pointer"
            style={{ color: "var(--text-secondary)" }}
            title="删除"
          >
            <TrashIcon className="w-3.5 h-3.5" />
          </button>
        </div>
      ))}
      <button
        onClick={addRow}
        className="flex items-center gap-1 text-xs mt-1 border-0 bg-transparent cursor-pointer px-0"
        style={{ color: "#007AFF" }}
      >
        <PlusIcon className="w-3.5 h-3.5" />
        <span>添加变量</span>
      </button>
    </div>
  );
}
