import { useState } from "react";
import type { InstallState } from "../hooks/useInstallState";
import { useToast } from "./Toast";

/** Modal for subscribing to a remote MCP config URL. On success the source's
 *  servers join the catalog. Mirrors AddAgentDialog's glass styling. */
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
      toast.show({ kind: "success", msg: `已订阅：${v.name}（${v.server_count} 个 server）` });
      onClose();
    } catch (e) {
      toast.show({ kind: "error", msg: "订阅失败：" + (Array.isArray(e) ? e.join("; ") : String(e)) });
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
    <div
      className="fixed inset-0 flex items-center justify-center z-40"
      style={{ background: "rgba(0,0,0,.3)", backdropFilter: "blur(8px)", WebkitBackdropFilter: "blur(8px)" }}
      onClick={onClose}
    >
      <div
        className="flex flex-col w-[520px] max-h-[82vh] rounded-mac-lg overflow-hidden"
        style={{
          background: "var(--surface-overlay)",
          backdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          WebkitBackdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          border: "1px solid var(--glass-border)",
          boxShadow: "var(--shadow-sheet), var(--glass-highlight)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-start gap-4 px-6 py-5" style={{ borderBottom: "1px solid var(--border-hairline)" }}>
          <div
            className="w-11 h-11 rounded-mac flex-shrink-0 flex items-center justify-center text-white text-2xl font-semibold leading-none"
            style={{ background: "linear-gradient(135deg, var(--color-brand-gold), var(--color-brand-coral), var(--color-brand-magenta))" }}
          >
            ☁
          </div>
          <div className="flex-1 min-w-0">
            <h2 className="text-base font-semibold m-0 mb-1" style={{ color: "var(--text-primary)" }}>
              订阅远程来源
            </h2>
            <p className="text-xs m-0 leading-relaxed" style={{ color: "var(--text-secondary)" }}>
              填写一个指向 MCP 配置文件的 URL（JSON / TOML）。抓取后会缓存到本地，其中的 server 会加入目录。
            </p>
          </div>
          <button
            onClick={onClose}
            className="flex-shrink-0 w-7 h-7 rounded-full flex items-center justify-center border-0 cursor-pointer mt-0.5"
            style={{ background: "var(--border-hairline)", color: "var(--text-secondary)" }}
          >
            <span className="text-xs font-medium">✕</span>
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-6 py-5 space-y-4">
          <div>
            <label className="text-xs font-medium block mb-1.5" style={{ color: "var(--text-secondary)" }}>
              配置文件 URL <span style={{ color: "#FF375F" }}>*</span>
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
              名称（可选）
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
          <p className="text-[11px] m-0 leading-relaxed" style={{ color: "var(--text-secondary)" }}>
            支持标准 <code style={{ fontFamily: "var(--font-mono)" }}>mcpServers</code> 配置，或 MUX 目录数组（带描述/标签）。
          </p>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-6 py-4" style={{ borderTop: "1px solid var(--border-hairline)" }}>
          <button onClick={onClose} className="btn-ghost">取消</button>
          <button disabled={!canSubmit} onClick={submit} className="btn-primary">
            {busy ? "订阅中…" : "订阅"}
          </button>
        </div>
      </div>
    </div>
  );
}
