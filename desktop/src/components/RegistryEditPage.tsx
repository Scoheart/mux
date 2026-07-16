import { useMemo, useState } from "react";
import type { InstallState } from "../hooks/useInstallState";
import type { RegistryEntry } from "../lib/types";
import { upsertRegistry, deleteRegistry, resyncEntry } from "../lib/api";
import { keyOf, transportOf, type Transport } from "../lib/mcp";
import { EnvEditor } from "./EnvEditor";
import { Modal, ModalHeader } from "./ui";
import { SaveIcon, RefreshIcon } from "./icons";
import { useToast } from "./Toast";

interface RegistryEditPageProps {
  state: InstallState;
  /** Entry name to edit; null = create a new entry. */
  name: string | null;
  /** Which transport variant to edit (a name can have both stdio + http). */
  transport?: Transport;
  onBack: () => void;
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
  const [repo, setRepo] = useState(existing?.repo ?? "");

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
      const synced = await upsertRegistry(draft);
      // Renamed (or changed transport) a custom entry → the new name is written
      // above; remove the old entry so it doesn't linger as a duplicate.
      if (existing && isCustom && keyOf(existing) !== draftKey) {
        await deleteRegistry(existing.name, transportOf(existing));
      }
      // Saving auto-synced the new config into installed agents — refresh both
      // the catalog and the install scan so usage/customized flags stay accurate.
      await Promise.all([refreshRegistry(), rescan()]);
      toast.show({
        kind: "success",
        msg:
          synced.length > 0
            ? `已保存 ${serverName.trim()}，并自动同步到 ${synced.length} 个 agent（${synced.join(", ")}）`
            : `已保存 ${serverName.trim()}`,
      });
      onBack();
    } catch (err) {
      // The catalog write happens before Agent propagation. Refresh from disk
      // even on failure so a partial sync is never hidden behind stale UI.
      await Promise.all([refreshRegistry().catch(console.error), rescan().catch(console.error)]);
      const message = String(err);
      toast.show({
        kind: "error",
        msg: message.includes("catalog saved, but Agent sync failed")
          ? `已保存 ${serverName.trim()}，但部分 Agent 同步失败: ${message}`
          : `保存失败: ${message}`,
      });
    } finally {
      setSaving(false);
    }
  };

  // Explicit re-sync: push this entry's *current saved* config into the agents
  // that have it installed. Safe by default (skips hand-customized installs);
  // if any were skipped, offer to force-overwrite.
  const handleResync = async () => {
    if (!existing || saving) return;
    const t = transportOf(existing);
    setSaving(true);
    try {
      let out = await resyncEntry(existing.name, t, false);
      if (out.skipped_customized.length > 0) {
        const ok = window.confirm(
          `${out.skipped_customized.length} 个 agent 的配置被手改过（${out.skipped_customized.join(", ")}），是否强制覆盖为当前配置？`
        );
        if (ok) out = await resyncEntry(existing.name, t, true);
      }
      await rescan();
      toast.show({
        kind: "success",
        msg:
          out.synced.length > 0
            ? `已同步到 ${out.synced.length} 个 agent（${out.synced.join(", ")}）`
            : "没有需要同步的已安装 agent",
      });
    } catch (err) {
      toast.show({ kind: "error", msg: `同步失败: ${String(err)}` });
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
    <Modal width={720} maxHeight="90vh" onClose={onBack}>
      <ModalHeader
        glyph={(serverName || "M")[0]?.toUpperCase()}
        title={isNew ? "新建 MCP" : "编辑 MCP"}
        subtitle={transport === "stdio" ? "stdio · 全局配置" : "HTTP · 全局配置"}
        onClose={onBack}
      />
      <div className="mux-mcp-form">
            {/* Name + description */}
            <div className="flex gap-4 mb-4">
              <div className="flex-1 min-w-0">
                <label className={labelCls} style={labelStyle}>名称</label>
                <input
                  style={{ ...inputStyle, fontFamily: "var(--font-mono)" }}
                  value={serverName}
                  disabled={!isNew && !isCustom}
                  onChange={(e) => setServerName(e.target.value)}
                  placeholder="server-name"
                />
                {!isNew && isCustom && existing && serverName.trim() !== existing.name && (
                  <p className="text-[11px] mt-1" style={{ color: "var(--text-secondary)" }}>
                    改名会移除旧条目；已装到 agent 的旧名不会自动改。
                  </p>
                )}
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

      <footer className="mux-model-form-footer">
        {!isNew && isCustom && (
          <button
            onClick={handleRevert}
            disabled={saving}
            className="btn-danger"
            title="删除自定义，恢复内置默认"
          >
            恢复默认
          </button>
        )}
        {!isNew && existing && (
          <button
            onClick={handleResync}
            disabled={saving}
            className="btn-secondary"
            title="把当前保存的配置重新同步到已安装此 MCP 的 agent（全局）"
          >
            <RefreshIcon className="w-4 h-4" />
            重新同步
          </button>
        )}
        <div className="flex-1" />
        <button onClick={onBack} className="btn-ghost">
          取消
        </button>
        <button
          onClick={handleSave}
          disabled={!valid || saving}
          className="btn-primary"
        >
          <SaveIcon className="w-4 h-4" />
          {saving ? "保存中…" : "保存"}
        </button>
      </footer>
    </Modal>
  );
}
