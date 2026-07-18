import { useState } from "react";
import type { InstallState } from "../hooks/useInstallState";
import { useToast } from "./Toast";
import { DialogShell } from "./DialogShell";
import { formatError } from "../lib/format";

/** Official curated collection preset for the shared subscription flow. */
const OFFICIAL_SOURCE = {
  url: "https://raw.githubusercontent.com/Scoheart/mux/main/data/registry.json",
  name: "Mux 精选",
};

/** Add a remote MCP config source, optionally using the Mux curated preset. */
export function SubscribeDialog({
  state,
  onClose,
}: {
  state: InstallState;
  onClose: () => void;
}) {
  const [url, setUrl] = useState("");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const canSubmit = url.trim().length > 0 && !busy;

  const submit = async () => {
    if (!canSubmit) return;
    setBusy(true);
    try {
      const v = await state.subscribe(url.trim(), name.trim() || undefined);
      toast.show({ kind: "success", msg: `已订阅 ${v.name} · ${v.server_count} 项` });
      onClose();
    } catch (e) {
      toast.show({ kind: "error", msg: "订阅失败：" + formatError(e) });
    } finally {
      setBusy(false);
    }
  };

  const fieldStyle = {
    background: "var(--surface-app)",
    border: "1px solid var(--border-hairline)",
    color: "var(--text-primary)",
  } as const;

  return (
    <DialogShell
      kind="editor"
      size="md"
      title="添加订阅"
      subtitle="输入 MCP 配置 URL，或使用 Mux 精选。"
      busy={busy}
      onClose={onClose}
      footerEnd={
        <>
          <button onClick={onClose} disabled={busy} className="btn-ghost">取消</button>
          <button disabled={!canSubmit} onClick={submit} className="btn-primary">
            {busy ? "订阅中…" : "订阅"}
          </button>
        </>
      }
    >

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-6 py-5 space-y-4">
          <div>
            <button
              type="button"
              onClick={() => { setUrl(OFFICIAL_SOURCE.url); setName(OFFICIAL_SOURCE.name); }}
              className="text-[11px] px-2.5 py-1 rounded-full cursor-pointer border-0"
              style={{ background: "var(--surface-raised)", color: "var(--color-blue)" }}
              title="使用 Mux 精选"
            >
              Mux 精选
            </button>
          </div>
          <div>
            <label className="text-xs font-medium block mb-1.5" style={{ color: "var(--text-secondary)" }}>
              配置 URL <span style={{ color: "#FF375F" }}>*</span>
            </label>
            <input
              autoFocus
              className="w-full px-3 py-2 text-sm rounded-mac outline-none"
              style={{ ...fieldStyle, fontFamily: "var(--font-mono)" }}
              placeholder="https://example.com/mcp.json"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") submit(); }}
            />
          </div>
          <div>
            <label className="text-xs font-medium block mb-1.5" style={{ color: "var(--text-secondary)" }}>
              名称
            </label>
            <input
              className="w-full px-3 py-2 text-sm rounded-mac outline-none"
              style={fieldStyle}
              placeholder="留空则用 URL 主机名"
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") submit(); }}
            />
          </div>
        </div>

    </DialogShell>
  );
}
