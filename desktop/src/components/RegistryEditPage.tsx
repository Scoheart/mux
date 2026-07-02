import { useMemo, useState } from "react";
import type { InstallState } from "../hooks/useInstallState";
import type { RegistryEntry } from "../lib/types";
import { upsertRegistry, deleteRegistry } from "../lib/api";
import { keyOf, transportOf, type Transport } from "../lib/mcp";
import { EnvEditor } from "./EnvEditor";
import { Avatar } from "./ui";
import { ArrowLeftIcon, SaveIcon } from "./icons";
import { useToast } from "./Toast";

interface RegistryEditPageProps {
  state: InstallState;
  /** Entry name to edit; null = create a new entry. */
  name: string | null;
  /** Which transport variant to edit (a name can have both stdio + http). */
  transport?: Transport;
  onBack: () => void;
}

const labelCls = "text-xs font-semibold uppercase mb-1.5 block";
const labelStyle = { color: "var(--text-secondary)", letterSpacing: "0.06em" } as const;
const inputStyle = {
  background: "var(--surface-app)",
  border: "1px solid var(--border-hairline)",
  color: "var(--text-primary)",
  fontSize: 13,
  padding: "8px 12px",
  borderRadius: 8,
  outline: "none",
  width: "100%",
} as const;

export function RegistryEditPage({ state, name, transport: editTransport, onBack }: RegistryEditPageProps) {
  const { entries, customKeys, refreshRegistry, rescan } = state;
  const toast = useToast();

  const isNew = name === null;
  const existing = useMemo(
    () =>
      name
        ? entries.find(
            (e) => e.name === name && (e.config.http ? "http" : "stdio") === (editTransport ?? "stdio")
          ) ?? null
        : null,
    [entries, name, editTransport]
  );
  const isCustom = existing ? customKeys.has(keyOf(existing)) : false;

  const [serverName, setServerName] = useState(existing?.name ?? "");
  const [description, setDescription] = useState(existing?.description ?? "");
  const [tagsText, setTagsText] = useState((existing?.tags ?? []).join(", "));
  const [transport, setTransport] = useState<Transport>(existing?.config.http ? "http" : "stdio");

  const [command, setCommand] = useState(existing?.config.stdio?.command ?? "");
  const [argsText, setArgsText] = useState((existing?.config.stdio?.args ?? []).join("\n"));
  const [env, setEnv] = useState<Record<string, string>>(existing?.config.stdio?.env ?? {});

  const [httpType, setHttpType] = useState<string>(existing?.config.http?.type ?? "http");
  const [url, setUrl] = useState(existing?.config.http?.url ?? "");
  const [headers, setHeaders] = useState<Record<string, string>>(existing?.config.http?.headers ?? {});

  const [saving, setSaving] = useState(false);

  const compact = (o: Record<string, string>) => (Object.keys(o).length > 0 ? o : undefined);

  const buildEntry = (): RegistryEntry => ({
    name: serverName.trim(),
    description: description.trim(),
    tags: tagsText.split(",").map((t) => t.trim()).filter(Boolean),
    config:
      transport === "stdio"
        ? {
            stdio: {
              command: command.trim(),
              args: argsText.split("\n").map((a) => a.trim()).filter(Boolean),
              env: compact(env),
            },
          }
        : { http: { type: httpType.trim() || "http", url: url.trim(), headers: compact(headers) } },
    // Preserve a recorded origin across edits; brand-new entries are manual.
    origin: existing?.origin ?? { kind: "manual" },
  });

  const valid =
    serverName.trim().length > 0 &&
    (transport === "stdio" ? command.trim().length > 0 : url.trim().length > 0);

  const handleSave = async () => {
    if (!valid || saving) return;
    const draft = buildEntry();
    const draftKey = keyOf(draft);
    // Block a name+transport collision with a *different* entry (same name with
    // another transport is allowed — that's the whole point of composite keys).
    if (entries.some((e) => keyOf(e) === draftKey && e !== existing)) {
      toast.show({ kind: "error", msg: `已存在同名同传输方式的 MCP: ${draft.name} (${transport})` });
      return;
    }
    setSaving(true);
    try {
      await upsertRegistry(draft);
      // The edit may have propagated into installed agents — refresh both the
      // catalog and the install scan so usage/customized flags stay accurate.
      await Promise.all([refreshRegistry(), rescan()]);
      toast.show({ kind: "success", msg: `已保存 ${serverName.trim()}` });
      onBack();
    } catch (err) {
      toast.show({ kind: "error", msg: `保存失败: ${String(err)}` });
    } finally {
      setSaving(false);
    }
  };

  const handleRevert = async () => {
    if (!name || saving || !existing) return;
    setSaving(true);
    try {
      await deleteRegistry(name, transportOf(existing));
      await Promise.all([refreshRegistry(), rescan()]);
      toast.show({ kind: "success", msg: `已恢复默认: ${name}` });
      onBack();
    } catch (err) {
      toast.show({ kind: "error", msg: `恢复失败: ${String(err)}` });
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex flex-col h-full" style={{ background: "var(--surface-app)" }}>
      {/* Header */}
      <header
        className="flex-shrink-0 flex items-center gap-3 px-5"
        style={{ height: 56, borderBottom: "1px solid var(--border-hairline)" }}
      >
        <button
          onClick={onBack}
          className="flex items-center justify-center w-9 h-9 rounded-mac cursor-pointer"
          style={{ background: "var(--surface-raised)", border: "1px solid var(--border-hairline)", color: "var(--text-primary)" }}
          title="返回"
          aria-label="返回"
        >
          <ArrowLeftIcon className="w-4 h-4" />
        </button>
        <h1 className="text-lg font-semibold m-0" style={{ color: "var(--text-primary)" }}>
          {isNew ? "新建 MCP" : "编辑 MCP"}
        </h1>
      </header>

      {/* Body */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        <div className="max-w-3xl mx-auto px-6 py-7">
          <div
            className="rounded-mac-lg p-6"
            style={{ background: "var(--surface-raised)", border: "1px solid var(--border-hairline)" }}
          >
            {/* Centered icon */}
            <div className="flex justify-center mb-6">
              <Avatar seed={serverName || "?"} size={64} />
            </div>

            {/* Name + description */}
            <div className="flex gap-4 mb-4">
              <div className="flex-1 min-w-0">
                <label className={labelCls} style={labelStyle}>名称</label>
                <input
                  style={{ ...inputStyle, fontFamily: "var(--font-mono)" }}
                  value={serverName}
                  disabled={!isNew}
                  onChange={(e) => setServerName(e.target.value)}
                  placeholder="server-name"
                />
              </div>
              <div className="flex-[1.6] min-w-0">
                <label className={labelCls} style={labelStyle}>描述</label>
                <input
                  style={inputStyle}
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="一句话描述"
                />
              </div>
            </div>

            {/* Tags */}
            <div className="mb-4">
              <label className={labelCls} style={labelStyle}>标签（逗号分隔）</label>
              <input
                style={inputStyle}
                value={tagsText}
                onChange={(e) => setTagsText(e.target.value)}
                placeholder="official, builtin"
              />
            </div>

            {/* Transport */}
            <div className="mb-4">
              <label className={labelCls} style={labelStyle}>传输方式</label>
              <div className="mux-seg">
                <button className="mux-seg-item" data-active={transport === "stdio" ? "true" : undefined} onClick={() => setTransport("stdio")}>
                  stdio
                </button>
                <button className="mux-seg-item" data-active={transport === "http" ? "true" : undefined} onClick={() => setTransport("http")}>
                  http / sse
                </button>
              </div>
            </div>

            {transport === "stdio" ? (
              <>
                <div className="mb-4">
                  <label className={labelCls} style={labelStyle}>命令 command</label>
                  <input
                    style={{ ...inputStyle, fontFamily: "var(--font-mono)" }}
                    value={command}
                    onChange={(e) => setCommand(e.target.value)}
                    placeholder="npx"
                  />
                </div>
                <div className="mb-4">
                  <label className={labelCls} style={labelStyle}>参数 args（每行一个）</label>
                  <textarea
                    style={{ ...inputStyle, fontFamily: "var(--font-mono)", minHeight: 80, resize: "vertical" }}
                    value={argsText}
                    onChange={(e) => setArgsText(e.target.value)}
                    placeholder={"-y\n@modelcontextprotocol/server-filesystem"}
                  />
                </div>
                <div>
                  <label className={labelCls} style={labelStyle}>环境变量 env</label>
                  <EnvEditor value={env} onChange={setEnv} />
                </div>
              </>
            ) : (
              <>
                <div className="mb-4">
                  <label className={labelCls} style={labelStyle}>类型 type</label>
                  <div className="mux-seg mb-2">
                    {["http", "sse", "streamable-http"].map((t) => (
                      <button
                        key={t}
                        className="mux-seg-item"
                        data-active={httpType === t ? "true" : undefined}
                        onClick={() => setHttpType(t)}
                      >
                        {t}
                      </button>
                    ))}
                  </div>
                  <input
                    style={{ ...inputStyle, fontFamily: "var(--font-mono)" }}
                    value={httpType}
                    onChange={(e) => setHttpType(e.target.value)}
                    placeholder="http / sse / streamable-http / 自定义"
                  />
                  <p className="text-[11px] mt-1" style={{ color: "var(--text-secondary)" }}>
                    可点上面预设，或直接输入任意 type（如 streamable-http）。写入配置文件的就是这里的原值。
                  </p>
                </div>
                <div className="mb-4">
                  <label className={labelCls} style={labelStyle}>URL</label>
                  <input
                    style={{ ...inputStyle, fontFamily: "var(--font-mono)" }}
                    value={url}
                    onChange={(e) => setUrl(e.target.value)}
                    placeholder="https://example.com/mcp"
                  />
                </div>
                <div>
                  <label className={labelCls} style={labelStyle}>请求头 headers</label>
                  <EnvEditor value={headers} onChange={setHeaders} />
                </div>
              </>
            )}
          </div>
        </div>
      </div>

      {/* Sticky footer */}
      <footer
        className="flex-shrink-0 flex items-center gap-3 px-6"
        style={{ height: 64, borderTop: "1px solid var(--border-hairline)", background: "var(--surface-sidebar)" }}
      >
        {!isNew && isCustom && (
          <button
            onClick={handleRevert}
            disabled={saving}
            className="text-sm px-3 py-2 rounded-mac border-0 cursor-pointer"
            style={{ background: "transparent", color: "#FF3B30" }}
            title="删除自定义，恢复内置默认"
          >
            恢复默认
          </button>
        )}
        <div className="flex-1" />
        <button
          onClick={onBack}
          className="text-sm px-4 py-2 rounded-mac cursor-pointer"
          style={{ background: "var(--surface-raised)", border: "1px solid var(--border-hairline)", color: "var(--text-primary)" }}
        >
          取消
        </button>
        <button
          onClick={handleSave}
          disabled={!valid || saving}
          className="flex items-center gap-1.5 text-sm font-medium px-5 py-2 rounded-mac border-0"
          style={{
            background: !valid || saving ? "var(--border-hairline)" : "#007AFF",
            color: !valid || saving ? "var(--text-secondary)" : "#fff",
            cursor: !valid || saving ? "default" : "pointer",
          }}
        >
          <SaveIcon className="w-4 h-4" />
          {saving ? "保存中…" : "保存"}
        </button>
      </footer>
    </div>
  );
}
