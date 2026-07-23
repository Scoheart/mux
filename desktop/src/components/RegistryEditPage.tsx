import { useMemo, useState } from "react";
import type { InstallState } from "../hooks/useInstallState";
import type { ConsumptionState } from "../hooks/useConsumptionState";
import type { RegistryEntry } from "../lib/types";
import { keyOf, type Transport } from "../lib/mcp";
import { EnvEditor } from "./EnvEditor";
import { DialogShell } from "./DialogShell";
import { ResourceInspector } from "./ResourceWorkspace";
import { SaveIcon } from "./icons";
import { useToast } from "./Toast";
import { Avatar } from "./ui";

interface RegistryEditPageProps {
  state: InstallState;
  consumptionState: ConsumptionState;
  /** Entry name to edit; null = create a new entry. */
  name: string | null;
  /** Exact catalog entry when editing from an Inspector, including shadowed copies. */
  entry?: RegistryEntry;
  /** Which transport variant to edit (a name can have both stdio + http). */
  transport?: Transport;
  onBack: () => void;
  /** Existing MCPs can edit inside the already-open resource Inspector. */
  presentation?: "dialog" | "inspector";
}

const labelCls = "text-xs font-semibold mb-1.5 block";
const labelStyle = { color: "var(--text-secondary)", letterSpacing: 0 } as const;
const inputStyle = {
  background: "var(--surface-raised)",
  border: "1px solid var(--border-hairline)",
  color: "var(--text-primary)",
  fontSize: 13,
  padding: "8px 12px",
  borderRadius: 8,
  outline: "none",
  width: "100%",
} as const;

export function RegistryEditPage({
  state,
  consumptionState,
  name,
  entry,
  transport: editTransport,
  onBack,
  presentation = "dialog",
}: RegistryEditPageProps) {
  const { entries, customKeys } = state;
  const toast = useToast();

  const isNew = name === null;
  const existing = useMemo(
    () =>
      entry ??
      (name
        ? entries.find(
            (e) => e.name === name && (e.config.http ? "http" : "stdio") === (editTransport ?? "stdio")
          ) ?? null
        : null),
    [entries, entry, name, editTransport]
  );
  const isCustom = existing ? customKeys.has(keyOf(existing)) : false;

  const [serverName, setServerName] = useState(existing?.name ?? "");
  const [description, setDescription] = useState(existing?.description ?? "");
  const [transport, setTransport] = useState<Transport>(existing?.config.http ? "http" : "stdio");

  const [command, setCommand] = useState(existing?.config.stdio?.command ?? "");
  const [argsText, setArgsText] = useState((existing?.config.stdio?.args ?? []).join("\n"));
  const [env, setEnv] = useState<Record<string, string>>(existing?.config.stdio?.env ?? {});

  const [httpType, setHttpType] = useState<string>(existing?.config.http?.type ?? "http");
  const [url, setUrl] = useState(existing?.config.http?.url ?? "");
  const [headers, setHeaders] = useState<Record<string, string>>(existing?.config.http?.headers ?? {});
  const [repo, setRepo] = useState(existing?.repo ?? "");

  const [saving, setSaving] = useState(false);

  const compact = (o: Record<string, string>) => (Object.keys(o).length > 0 ? o : undefined);

  const buildEntry = (): RegistryEntry => ({
    name: serverName.trim(),
    description: description.trim(),
    // Tags describe provenance supplied by curated or imported assets. They are
    // intentionally not user-editable, but must survive an edit unchanged.
    tags: existing?.tags ?? [],
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
    repo: repo.trim() || undefined,
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
      await consumptionState.planUpdate({
        domain: "mcp",
        existing_key: existing ? keyOf(existing) : undefined,
        entry: draft,
      });
      toast.show({ kind: "success", msg: "已准备好更改，请确认后应用。" });
      onBack();
    } catch (err) {
      toast.show({ kind: "error", msg: `无法保存：${String(err)}` });
    } finally {
      setSaving(false);
    }
  };

  const handleRevert = async () => {
    if (!name || saving || !existing) return;
    setSaving(true);
    try {
      const sourceId = existing.origin?.source ?? existing.origin?.kind;
      await consumptionState.planDelete(
        { domain: "mcp", key: keyOf(existing) },
        sourceId,
      );
      toast.show({ kind: "success", msg: `已准备恢复 ${name} 的默认设置，请确认后应用。` });
      onBack();
    } catch (err) {
      toast.show({ kind: "error", msg: `恢复失败: ${String(err)}` });
    } finally {
      setSaving(false);
    }
  };

  const form = (
    <div className="mux-mcp-form">
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

            {/* Repo / homepage */}
            <div className="mb-4">
              <label className={labelCls} style={labelStyle}>仓库 / 主页（可选）</label>
              <input
                style={{ ...inputStyle, fontFamily: "var(--font-mono)" }}
                value={repo}
                onChange={(e) => setRepo(e.target.value)}
                placeholder="https://github.com/owner/repo"
              />
            </div>

            {/* Transport */}
            <div className="mb-4">
              <label className={labelCls} style={labelStyle}>传输方式</label>
              <div className="mux-seg">
                <button disabled={!isNew} className="mux-seg-item" data-active={transport === "stdio" ? "true" : undefined} onClick={() => setTransport("stdio")}>
                  stdio
                </button>
                <button disabled={!isNew} className="mux-seg-item" data-active={transport === "http" ? "true" : undefined} onClick={() => setTransport("http")}>
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
                    可点预设或输入任意 type（如 streamable-http），原值写入配置。
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
  );

  const footerStart = !isNew && isCustom ? (
    <button onClick={handleRevert} disabled={saving} className="btn-danger" title="删除自定义，恢复内置默认">
      恢复默认
    </button>
  ) : null;
  const footerEnd = (
    <>
      <button onClick={onBack} disabled={saving} className="btn-ghost">取消</button>
      <button onClick={handleSave} disabled={!valid || saving} className="btn-primary">
        <SaveIcon className="w-4 h-4" />
        {saving ? "保存中…" : "保存"}
      </button>
    </>
  );

  if (presentation === "inspector" && existing) {
    return (
      <ResourceInspector
        title={existing.name}
        avatar={<Avatar seed={existing.name} kind="mcp" size={40} />}
        subtitle={`编辑 · ${transport === "stdio" ? "stdio" : "HTTP"} · 全局配置`}
        onClose={onBack}
        footer={
          <>
            {footerStart}
            <div className="flex-1" />
            {footerEnd}
          </>
        }
      >
        {form}
      </ResourceInspector>
    );
  }

  return (
    <DialogShell
      kind="editor"
      size="lg"
      title={isNew ? "新建 MCP" : "编辑 MCP"}
      subtitle={transport === "stdio" ? "stdio · 全局配置" : "HTTP · 全局配置"}
      busy={saving}
      onClose={onBack}
      footerStart={footerStart}
      footerEnd={footerEnd}
    >
      {form}
    </DialogShell>
  );
}
