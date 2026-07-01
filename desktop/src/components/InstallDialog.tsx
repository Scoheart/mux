import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { listAgents, previewInstall, applyInstall } from "../lib/api";
import type { RegistryEntry, AgentInfo, PlannedWrite, InstallRequest, PatchInput } from "../lib/types";
import { transportOf } from "../lib/mcp";
import { FolderIcon, CheckIcon } from "./icons";
import { EnvEditor } from "./EnvEditor";
import { useToast } from "./Toast";

const SCOPES = [
  { value: "global", label: "全局" },
  { value: "project", label: "项目" },
] as const;

export function InstallDialog({ entry, onClose }: { entry: RegistryEntry; onClose: () => void }) {
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [scope, setScope] = useState<"global" | "project">("global");
  const [projectDir, setProjectDir] = useState<string>("");
  const [selected, setSelected] = useState<Record<string, boolean>>({});
  const [overrides, setOverrides] = useState<Record<string, PatchInput>>({});
  const [preview, setPreview] = useState<PlannedWrite[] | null>(null);
  const toast = useToast();

  useEffect(() => { listAgents().then(setAgents).catch(console.error); }, []);

  const eligible = agents.filter((a) => (scope === "global" ? a.has_global : a.has_project));
  const chosen = eligible.filter((a) => selected[a.id]).map((a) => a.id);

  const req = (): InstallRequest => ({
    server_name: entry.name, transport: transportOf(entry), scope, agents: chosen,
    project_dir: scope === "project" ? projectDir : undefined,
    overrides,
  });

  const pickFolder = async () => {
    const dir = await open({ directory: true });
    if (typeof dir === "string") setProjectDir(dir);
  };

  const doPreview = async () => {
    try { setPreview(await previewInstall(req())); }
    catch (e) { toast.show({ kind: "error", msg: "预览失败：" + String(e) }); }
  };

  const doApply = async () => {
    try {
      await applyInstall(req());
      toast.show({ kind: "success", msg: "已应用" });
      onClose();
    } catch (e) {
      toast.show({ kind: "error", msg: "应用失败：" + (Array.isArray(e) ? e.join("; ") : String(e)) });
    }
  };

  const canSubmit = chosen.length > 0 && (scope === "global" || Boolean(projectDir));
  const monogram = entry.name[0]?.toUpperCase() ?? "?";

  return (
    <div
      className="fixed inset-0 flex items-center justify-center z-40"
      style={{ background: "rgba(0,0,0,.3)", backdropFilter: "blur(8px)", WebkitBackdropFilter: "blur(8px)" }}
      onClick={onClose}
    >
      <div
        className="flex flex-col w-[560px] max-h-[82vh] rounded-mac-lg overflow-hidden"
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
        <div
          className="flex items-start gap-4 px-6 py-5"
          style={{ borderBottom: "1px solid var(--border-hairline)" }}
        >
          <div
            className="w-11 h-11 rounded-mac flex-shrink-0 flex items-center justify-center text-white text-lg font-semibold"
            style={{ background: "linear-gradient(135deg, #007AFF, #5AC8FA)" }}
          >
            {monogram}
          </div>
          <div className="flex-1 min-w-0">
            <h2 className="text-base font-semibold m-0 mb-1" style={{ color: "var(--text-primary)" }}>
              安装 {entry.name}
            </h2>
            <p className="text-xs m-0 leading-relaxed" style={{ color: "var(--text-secondary)" }}>
              {entry.description}
            </p>
          </div>
          <button
            onClick={onClose}
            className="flex-shrink-0 w-7 h-7 rounded-full flex items-center justify-center border-0 cursor-pointer mt-0.5"
            style={{
              background: "var(--border-hairline)",
              color: "var(--text-secondary)",
            }}
          >
            <span className="text-xs font-medium">✕</span>
          </button>
        </div>

        {/* Body — scrollable */}
        <div className="flex-1 overflow-y-auto px-6 py-5 space-y-5">

          {/* Scope segmented control */}
          <div>
            <label className="text-xs font-medium block mb-2" style={{ color: "var(--text-secondary)" }}>
              安装范围
            </label>
            <div
              className="inline-flex p-0.5 rounded-mac"
              style={{ background: "var(--surface-app)" }}
            >
              {SCOPES.map((s) => (
                <button
                  key={s.value}
                  onClick={() => setScope(s.value)}
                  className="px-4 py-1.5 text-sm rounded-[8px] border-0 cursor-pointer transition-all font-medium"
                  style={{
                    background: scope === s.value ? "var(--surface-raised)" : "transparent",
                    color: scope === s.value ? "var(--text-primary)" : "var(--text-secondary)",
                    boxShadow: scope === s.value ? "var(--shadow-card)" : "none",
                  }}
                >
                  {s.label}
                </button>
              ))}
            </div>

            {/* Project dir picker */}
            {scope === "project" && (
              <div className="mt-3 flex items-center gap-2">
                <button
                  onClick={pickFolder}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-mac border cursor-pointer transition-colors"
                  style={{
                    background: "var(--surface-raised)",
                    border: "1px solid var(--border-hairline)",
                    color: "var(--text-primary)",
                  }}
                >
                  <FolderIcon className="w-4 h-4" />
                  <span>选择项目目录…</span>
                </button>
                {projectDir && (
                  <span
                    className="text-xs px-2 py-1 rounded-mac truncate max-w-[200px]"
                    style={{
                      background: "var(--surface-app)",
                      color: "var(--text-secondary)",
                    }}
                    title={projectDir}
                  >
                    {projectDir.split("/").pop()}
                  </span>
                )}
              </div>
            )}
          </div>

          {/* Agent rows */}
          <div>
            <label className="text-xs font-medium block mb-2" style={{ color: "var(--text-secondary)" }}>
              目标 Agents
            </label>
            {eligible.length === 0 ? (
              <p className="text-sm" style={{ color: "var(--text-secondary)" }}>
                该 scope 下无可用 agent
              </p>
            ) : (
              <div className="space-y-2">
                {eligible.map((a: AgentInfo) => (
                  <div
                    key={a.id}
                    className="rounded-mac px-3 py-2.5"
                    style={{
                      background: "var(--surface-app)",
                      border: `1px solid ${selected[a.id] ? "#007AFF" : "var(--border-hairline)"}`,
                    }}
                  >
                    {/* Agent checkbox row */}
                    <label className="flex items-center gap-2.5 cursor-pointer">
                      {/* Custom checkbox */}
                      <input
                        type="checkbox"
                        className="sr-only"
                        checked={!!selected[a.id]}
                        onChange={(e) => setSelected((s) => ({ ...s, [a.id]: e.target.checked }))}
                      />
                      <span
                        className="w-4 h-4 rounded flex-shrink-0 border flex items-center justify-center transition-colors"
                        style={{
                          background: selected[a.id] ? "#007AFF" : "var(--surface-raised)",
                          borderColor: selected[a.id] ? "#007AFF" : "var(--border-hairline)",
                        }}
                      >
                        <CheckIcon
                          className="w-2.5 h-2.5"
                          style={{
                            color: "white",
                            opacity: selected[a.id] ? 1 : 0,
                            transition: "opacity 0.1s",
                          }}
                        />
                      </span>
                      <span className="text-sm font-medium" style={{ color: "var(--text-primary)" }}>
                        {a.id}
                      </span>
                      <span
                        className="text-[11px] px-1.5 py-0.5 rounded"
                        style={{
                          background: "var(--border-hairline)",
                          color: "var(--text-secondary)",
                        }}
                      >
                        {a.format}
                      </span>
                    </label>

                    {/* Env editor (expanded when checked) */}
                    {selected[a.id] && (
                      <div className="mt-2 pt-2" style={{ borderTop: "1px solid var(--border-hairline)" }}>
                        <span className="text-[11px]" style={{ color: "var(--text-secondary)" }}>
                          环境变量覆写
                        </span>
                        <EnvEditor
                          value={overrides[a.id]?.env ?? {}}
                          onChange={(env) =>
                            setOverrides((o) => ({ ...o, [a.id]: { ...o[a.id], env } }))
                          }
                        />
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Preview panel */}
          {preview && preview.length > 0 && (
            <div>
              <label className="text-xs font-medium block mb-2" style={{ color: "var(--text-secondary)" }}>
                预览（将写入）
              </label>
              <div className="space-y-2">
                {preview.map((p: PlannedWrite) => (
                  <div
                    key={p.agent}
                    className="rounded-mac overflow-hidden"
                    style={{ border: "1px solid var(--border-hairline)" }}
                  >
                    <div
                      className="px-3 py-2 text-xs font-medium"
                      style={{
                        background: "var(--surface-app)",
                        borderBottom: "1px solid var(--border-hairline)",
                        color: "var(--text-secondary)",
                        fontFamily: "var(--font-mono)",
                      }}
                    >
                      {p.agent} → {p.file_path}
                    </div>
                    <pre
                      className="m-0 px-3 py-2 overflow-x-auto"
                      style={{
                        fontSize: 11,
                        fontFamily: "var(--font-mono)",
                        color: "var(--text-primary)",
                        background: "var(--surface-raised)",
                        maxHeight: 180,
                        overflowY: "auto",
                      }}
                    >
                      {p.config_json}
                    </pre>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div
          className="flex items-center justify-end gap-2 px-6 py-4"
          style={{ borderTop: "1px solid var(--border-hairline)" }}
        >
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm rounded-mac border-0 cursor-pointer"
            style={{
              background: "transparent",
              color: "var(--text-secondary)",
            }}
          >
            取消
          </button>
          <button
            disabled={!canSubmit}
            onClick={doPreview}
            className="px-4 py-2 text-sm rounded-mac cursor-pointer transition-colors"
            style={{
              background: "transparent",
              border: "1px solid var(--border-hairline)",
              color: canSubmit ? "var(--text-primary)" : "var(--text-secondary)",
              opacity: canSubmit ? 1 : 0.4,
            }}
          >
            预览改动
          </button>
          <button
            disabled={!canSubmit}
            onClick={doApply}
            className="px-4 py-2 text-sm rounded-mac border-0 cursor-pointer font-medium"
            style={{
              background: canSubmit ? "#007AFF" : "#8E8E93",
              color: "#fff",
              opacity: canSubmit ? 1 : 0.4,
            }}
            onMouseEnter={(e) => { if (canSubmit) e.currentTarget.style.background = "#0066D6"; }}
            onMouseLeave={(e) => { e.currentTarget.style.background = "#007AFF"; }}
            onMouseDown={(e) => { if (canSubmit) e.currentTarget.style.background = "#0051A3"; }}
            onMouseUp={(e) => { e.currentTarget.style.background = canSubmit ? "#0066D6" : "#007AFF"; }}
          >
            应用
          </button>
        </div>
      </div>
    </div>
  );
}
