import { useState } from "react";
import type { InstallState } from "../hooks/useInstallState";
import { useToast } from "./Toast";

const EXAMPLE = `{
  "mcpServers": {
    "yunxiao": {
      "command": "npx",
      "args": ["-y", "alibabacloud-devops-mcp-server"],
      "env": { "YUNXIAO_ACCESS_TOKEN": "<YOUR_TOKEN>" }
    }
  }
}`;

/** Paste a config blob (a `mcpServers` JSON/TOML object, a bare name→config map,
 *  or a single server) — MUX recognizes the servers and adds them to 手动添加. */
export function PasteConfigDialog({
  state,
  onClose,
}: {
  state: InstallState;
  onClose: () => void;
}) {
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const submit = async () => {
    if (!text.trim() || busy) return;
    setBusy(true);
    try {
      const names = await state.importPaste(text);
      toast.show({ kind: "success", msg: `已添加 ${names.length} 个：${names.join("、")}` });
      onClose();
    } catch (e) {
      toast.show({ kind: "error", msg: "识别失败：" + (Array.isArray(e) ? e.join("; ") : String(e)) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 flex items-center justify-center z-40"
      style={{ background: "rgba(0,0,0,.3)", backdropFilter: "blur(8px)", WebkitBackdropFilter: "blur(8px)" }}
      onClick={onClose}
    >
      <div
        className="flex flex-col w-[560px] max-h-[86vh] rounded-mac-lg overflow-hidden"
        style={{
          background: "var(--surface-overlay)",
          backdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          WebkitBackdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          border: "1px solid var(--glass-border)",
          boxShadow: "var(--shadow-sheet), var(--glass-highlight)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start gap-4 px-6 py-5" style={{ borderBottom: "1px solid var(--border-hairline)" }}>
          <div
            className="w-11 h-11 rounded-mac flex-shrink-0 flex items-center justify-center text-white text-2xl font-semibold leading-none"
            style={{ background: "linear-gradient(135deg, var(--color-brand-gold), var(--color-brand-coral), var(--color-brand-magenta))" }}
          >
            ⧉
          </div>
          <div className="flex-1 min-w-0">
            <h2 className="text-base font-semibold m-0 mb-1" style={{ color: "var(--text-primary)" }}>
              粘贴配置
            </h2>
            <p className="text-xs m-0 leading-relaxed" style={{ color: "var(--text-secondary)" }}>
              直接粘贴一段 <code style={{ fontFamily: "var(--font-mono)" }}>mcpServers</code> 配置（JSON / TOML），MUX 会识别其中的 server 并加入「手动添加」。
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

        <div className="flex-1 overflow-y-auto px-6 py-5">
          <textarea
            autoFocus
            className="w-full rounded-mac outline-none"
            style={{
              background: "var(--surface-app)",
              border: "1px solid var(--border-hairline)",
              color: "var(--text-primary)",
              fontFamily: "var(--font-mono)",
              fontSize: 12.5,
              padding: "10px 12px",
              minHeight: 240,
              resize: "vertical",
              lineHeight: 1.5,
            }}
            placeholder={EXAMPLE}
            value={text}
            onChange={(e) => setText(e.target.value)}
          />
        </div>

        <div className="flex items-center justify-end gap-2 px-6 py-4" style={{ borderTop: "1px solid var(--border-hairline)" }}>
          <button onClick={onClose} className="btn-ghost">取消</button>
          <button disabled={!text.trim() || busy} onClick={submit} className="btn-primary">
            {busy ? "识别中…" : "识别并添加"}
          </button>
        </div>
      </div>
    </div>
  );
}
