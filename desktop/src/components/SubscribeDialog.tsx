import { useState } from "react";
import type { InstallState } from "../hooks/useInstallState";
import { useToast } from "./Toast";
import { Modal, ModalHeader } from "./ui";
import { formatError } from "../lib/format";

/** The official curated collection, subscribed as a remote source (the repo's
 *  raw registry.json). Kept here so the Sources page can pre-fill this dialog. */
export const OFFICIAL_SOURCE = {
  url: "https://raw.githubusercontent.com/Scoheart/mux/main/data/registry.json",
  name: "官方精选合集",
};

/** Modal for subscribing to a remote MCP config URL. On success the source's
 *  servers join the catalog. Mirrors AddAgentDialog's glass styling. Pass
 *  `initialUrl`/`initialName` to pre-fill (e.g. the official source). */
export function SubscribeDialog({
  state,
  onClose,
  initialUrl,
  initialName,
}: {
  state: InstallState;
  onClose: () => void;
  initialUrl?: string;
  initialName?: string;
}) {
  const [url, setUrl] = useState(initialUrl ?? "");
  const [name, setName] = useState(initialName ?? "");
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
    <Modal width={520} onClose={onClose}>
        <ModalHeader
          glyph="☁"
          title="订阅远程来源"
          subtitle={
            <>
              订阅一个<b>远程</b>配置源：填一个指向 MCP 配置文件的 URL（JSON / TOML）。MUX
              抓取后缓存一份，其中的 server 加入目录，之后可「刷新」重抓、随远端更新。（本机已有的文件请用「导入本地文件」。）
            </>
          }
          onClose={onClose}
        />

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-6 py-5 space-y-4">
          <div>
            <button
              type="button"
              onClick={() => { setUrl(OFFICIAL_SOURCE.url); setName(OFFICIAL_SOURCE.name); }}
              className="text-[11px] px-2.5 py-1 rounded-full cursor-pointer border-0"
              style={{ background: "var(--surface-raised)", color: "var(--color-blue)" }}
              title="填入官方精选合集的订阅地址"
            >
              使用官方精选合集
            </button>
          </div>
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
    </Modal>
  );
}
