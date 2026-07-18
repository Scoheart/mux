import { useState } from "react";
import { addAgent, updateAgent } from "../lib/api";
import type { AgentDefinitionInput, AgentInfo } from "../lib/types";
import { useToast } from "./Toast";
import { DialogShell } from "./DialogShell";
import { formatError } from "../lib/format";

const FORMATS = [
  { value: "json", label: "JSON" },
  { value: "toml", label: "TOML" },
  { value: "yaml", label: "YAML" },
] as const;

/** Register a custom global target or edit one already known to MUX. Built-in
 *  schemas are locked, so their edit mode exposes only the global path. */
export function AddAgentDialog({
  onClose,
  onAdded,
  existing,
}: {
  onClose: () => void;
  onAdded: () => Promise<unknown> | void;
  existing?: AgentInfo;
}) {
  const isEdit = !!existing;
  const schemaLocked = existing?.builtin ?? false;
  const [id, setId] = useState(existing?.id ?? "");
  const [format, setFormat] = useState<"json" | "toml" | "yaml">(
    existing?.format === "toml" || existing?.format === "yaml" ? existing.format : "json"
  );
  const [key, setKey] = useState(existing?.key ?? "mcpServers");
  const [global, setGlobal] = useState(existing?.global ?? "");
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const trimmedId = id.trim();
  const canSubmit =
    trimmedId.length > 0 &&
    key.trim().length > 0 &&
    global.trim().length > 0 &&
    !busy;

  const submit = async () => {
    if (!canSubmit) return;
    setBusy(true);
    const def: AgentDefinitionInput = {
      global: global.trim() || null,
      // Preserve legacy metadata when editing, but project scope is no longer
      // exposed by the product.
      project: existing?.project ?? null,
      format,
      key: key.trim(),
      enabled: existing?.enabled ?? true,
      builtin: existing?.builtin ?? false,
    };
    try {
      if (isEdit) {
        await updateAgent(trimmedId, def);
        toast.show({ kind: "success", msg: `已更新 agent：${trimmedId}` });
      } else {
        await addAgent(trimmedId, def);
        toast.show({ kind: "success", msg: `已添加 agent：${trimmedId}` });
      }
      await onAdded();
      onClose();
    } catch (e) {
      const verb = isEdit ? "更新" : "添加";
      toast.show({ kind: "error", msg: `${verb}失败：` + formatError(e) });
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
        title={schemaLocked ? "编辑全局路径" : isEdit ? "编辑 Agent" : "添加 Agent"}
        subtitle={existing?.name ?? "自定义 Agent"}
        busy={busy}
        onClose={onClose}
        footerEnd={
          <>
            <button onClick={onClose} disabled={busy} className="btn-ghost">取消</button>
            <button disabled={!canSubmit} onClick={submit} className="btn-primary">
              {busy ? (isEdit ? "保存中…" : "添加中…") : isEdit ? "保存" : "添加"}
            </button>
          </>
        }
      >

      {/* Body */}
      <div className="flex-1 overflow-y-auto px-6 py-5 space-y-4">
          {/* id */}
          <div>
            <label className="text-xs font-medium block mb-1.5" style={{ color: "var(--text-secondary)" }}>
              Agent ID <span style={{ color: "#FF375F" }}>*</span>
            </label>
            <input
              autoFocus={!isEdit}
              disabled={isEdit}
              className="w-full px-3 py-2 text-sm rounded-mac outline-none"
              style={{ ...fieldStyle, opacity: isEdit ? 0.6 : 1, cursor: isEdit ? "not-allowed" : "text" }}
              placeholder="例如：my-tool"
              value={id}
              onChange={(e) => setId(e.target.value)}
            />
          </div>

          {/* format + key */}
          <div className="flex gap-3">
            <div className="flex-shrink-0">
              <label className="text-xs font-medium block mb-1.5" style={{ color: "var(--text-secondary)" }}>
                格式
              </label>
              <div className="inline-flex p-0.5 rounded-mac" style={{ background: "var(--surface-app)" }}>
                {FORMATS.map((f) => (
                  <button
                    key={f.value}
                    disabled={schemaLocked}
                    onClick={() => setFormat(f.value)}
                    className="px-3 py-1.5 text-sm rounded-[8px] border-0 cursor-pointer transition-all font-medium"
                    style={{
                      background: format === f.value ? "var(--surface-raised)" : "transparent",
                      color: format === f.value ? "var(--text-primary)" : "var(--text-secondary)",
                      boxShadow: format === f.value ? "var(--shadow-card)" : "none",
                      cursor: schemaLocked ? "default" : "pointer",
                      opacity: schemaLocked && format !== f.value ? 0.35 : 1,
                    }}
                  >
                    {f.label}
                  </button>
                ))}
              </div>
            </div>
            <div className="flex-1 min-w-0">
              <label className="text-xs font-medium block mb-1.5" style={{ color: "var(--text-secondary)" }}>
                配置 Key <span style={{ color: "#FF375F" }}>*</span>
              </label>
              <input
                disabled={schemaLocked}
                className="w-full px-3 py-2 text-sm rounded-mac outline-none"
                style={{
                  ...fieldStyle,
                  fontFamily: "var(--font-mono)",
                  opacity: schemaLocked ? 0.6 : 1,
                  cursor: schemaLocked ? "not-allowed" : "text",
                }}
                placeholder="mcpServers"
                value={key}
                onChange={(e) => setKey(e.target.value)}
              />
            </div>
          </div>

          {/* global path */}
          <div>
            <label className="text-xs font-medium block mb-1.5" style={{ color: "var(--text-secondary)" }}>
              全局配置路径 <span style={{ color: "#FF375F" }}>*</span>
            </label>
            <input
              className="w-full px-3 py-2 text-sm rounded-mac outline-none"
              style={{ ...fieldStyle, fontFamily: "var(--font-mono)" }}
              placeholder="~/.mytool/mcp.json"
              value={global}
              onChange={(e) => setGlobal(e.target.value)}
            />
          </div>

      </div>

    </DialogShell>
  );
}
