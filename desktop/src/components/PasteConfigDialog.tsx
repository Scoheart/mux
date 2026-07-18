import { useState } from "react";
import type { InstallState } from "../hooks/useInstallState";
import { useToast } from "./Toast";
import { DialogShell } from "./DialogShell";
import { formatError } from "../lib/format";

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
      toast.show({ kind: "error", msg: "识别失败：" + formatError(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <DialogShell
          kind="editor"
          size="md"
          title="粘贴配置"
          subtitle={
            <>
              粘贴一段 <code style={{ fontFamily: "var(--font-mono)" }}>mcpServers</code> 配置，server 加入「手动添加」。
            </>
          }
          busy={busy}
          onClose={onClose}
          footerEnd={
            <>
              <button onClick={onClose} disabled={busy} className="btn-ghost">取消</button>
              <button disabled={!text.trim() || busy} onClick={submit} className="btn-primary">
                {busy ? "识别中…" : "识别并添加"}
              </button>
            </>
          }
        >

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

    </DialogShell>
  );
}
