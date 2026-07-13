import { useState } from "react";
import { addAgent, updateAgent } from "../lib/api";
import type { AgentDefinitionInput, AgentInfo } from "../lib/types";
import { useToast } from "./Toast";
import { Modal, ModalHeader } from "./ui";
import { formatError } from "../lib/format";

const FORMATS = [
  { value: "json", label: "JSON" },
  { value: "toml", label: "TOML" },
] as const;

/** Modal form for registering a new custom agent, or editing an existing one's
 *  global config path/format/key (persisted to settings.agents). Pass `existing` to
 *  open in edit mode (the id is then read-only). */
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
  const [id, setId] = useState(existing?.id ?? "");
  const [format, setFormat] = useState<"json" | "toml">(
    existing?.format === "toml" ? "toml" : "json"
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
      builtin: false,
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
    <Modal width={520} onClose={onClose}>
        <ModalHeader
          glyph={isEdit ? "✎" : "+"}
          title={isEdit ? "编辑 Agent" : "添加 Agent"}
          subtitle={
            isEdit
              ? "修改全局配置路径、格式和 Key。"
              : "注册自定义工具及其全局 MCP 配置。"
          }
          onClose={onClose}
        />

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
                    onClick={() => setFormat(f.value)}
                    className="px-3 py-1.5 text-sm rounded-[8px] border-0 cursor-pointer transition-all font-medium"
                    style={{
                      background: format === f.value ? "var(--surface-raised)" : "transparent",
                      color: format === f.value ? "var(--text-primary)" : "var(--text-secondary)",
                      boxShadow: format === f.value ? "var(--shadow-card)" : "none",
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
                className="w-full px-3 py-2 text-sm rounded-mac outline-none"
                style={{ ...fieldStyle, fontFamily: "var(--font-mono)" }}
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

        {/* Footer */}
        <div
          className="flex items-center justify-end gap-2 px-6 py-4"
          style={{ borderTop: "1px solid var(--border-hairline)" }}
        >
          <button onClick={onClose} className="btn-ghost">
            取消
          </button>
          <button disabled={!canSubmit} onClick={submit} className="btn-primary">
            {busy ? (isEdit ? "保存中…" : "添加中…") : isEdit ? "保存" : "添加"}
          </button>
        </div>
    </Modal>
  );
}
