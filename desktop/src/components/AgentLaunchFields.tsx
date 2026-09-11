import { open } from "@tauri-apps/plugin-dialog";
import type { AgentLaunchInfo, LaunchTarget } from "../lib/agentLaunch";
import { formatError } from "../lib/format";
import { FolderIcon, LanguageIcon, TerminalIcon } from "./icons";

export interface AgentLaunchDraft {
  kind: LaunchTarget["kind"];
  app: string;
  command: string;
  args: string;
  url: string;
  resetDefault: boolean;
}

export function createAgentLaunchDraft(info: AgentLaunchInfo): AgentLaunchDraft {
  const target = info.configured_target ?? info.resolved_target;
  return { kind: target?.kind ?? info.kind ?? "app", app: target?.kind === "app" ? target.path : "",
    command: target?.kind === "cli" ? target.command : "", args: target?.kind === "cli" ? target.args.join("\n") : "",
    url: target?.kind === "web" ? target.url : "", resetDefault: false };
}

export function agentLaunchDraftTarget(draft: AgentLaunchDraft): LaunchTarget | null {
  if (draft.resetDefault) return null;
  if (draft.kind === "app") return { kind: "app", path: draft.app };
  if (draft.kind === "web") return { kind: "web", url: draft.url.trim() };
  return { kind: "cli", command: draft.command.trim(), args: draft.args.split("\n").filter(Boolean) };
}

export function agentLaunchDraftValid(draft: AgentLaunchDraft) {
  return draft.resetDefault || Boolean(draft.kind === "app" ? draft.app.trim() : draft.kind === "cli" ? draft.command.trim() : draft.url.trim());
}

export function agentLaunchDraftChanged(info: AgentLaunchInfo, draft: AgentLaunchDraft) {
  if (draft.resetDefault) return info.configured_target !== null;
  return JSON.stringify(agentLaunchDraftTarget(draft)) !== JSON.stringify(agentLaunchDraftTarget(createAgentLaunchDraft(info)));
}

export function AgentLaunchFields({ info, draft, disabled, onChange, onBusyChange, onError }: {
  info: AgentLaunchInfo; draft: AgentLaunchDraft; disabled: boolean;
  onChange(draft: AgentLaunchDraft): void; onBusyChange(busy: boolean): void; onError(message: string): void;
}) {
  const change = (patch: Partial<AgentLaunchDraft>) => onChange({ ...draft, resetDefault: false, ...patch });
  const browse = async () => {
    if (disabled) return;
    onBusyChange(true); onError("");
    try {
      const value = await open({ title: draft.kind === "app" ? "选择 Agent 应用" : "选择可执行程序", multiple: false, directory: false,
        ...(draft.kind === "app" ? { defaultPath: "/Applications", filters: [{ name: "应用程序", extensions: ["app"] }] } : {}) });
      if (value) change(draft.kind === "app" ? { app: value } : { command: value });
    } catch (error) { onError(formatError(error)); }
    finally { onBusyChange(false); }
  };
  return <fieldset className="mux-launch-settings mux-launch-fields" disabled={disabled}>
    <div className="mux-launch-field-modes">
      <div className="mux-seg" role="group" aria-label="启动方式">
        {([['app', '应用', FolderIcon], ['cli', 'CLI', TerminalIcon], ['web', '网页', LanguageIcon]] as const).map(([kind, label, Icon]) =>
          <button key={kind} type="button" className="mux-seg-item" aria-pressed={!draft.resetDefault && draft.kind === kind}
            data-active={!draft.resetDefault && draft.kind === kind || undefined} onClick={() => change({ kind })}>
            <Icon className="w-3.5 h-3.5" />{label}
          </button>)}
      </div>
      {(info.configured_target || agentLaunchDraftChanged(info, draft)) && <button type="button" className="mux-launch-reset"
        onClick={() => onChange({ ...createAgentLaunchDraft(info), resetDefault: true })}>恢复默认</button>}
    </div>
    {draft.resetDefault ? <p className="mux-launch-hint">保存后使用 Agent 默认启动方式</p> : <>
      {draft.kind === "web" ? <label>网址<input className="mux-dialog-input" value={draft.url} placeholder="https://" onChange={(event) => change({ url: event.target.value })} /></label>
        : <label>{draft.kind === "app" ? "应用" : "可执行程序"}<div className="mux-launch-path-input">
          <input className="mux-dialog-input" value={draft.kind === "app" ? draft.app : draft.command} placeholder={draft.kind === "app" ? "选择 .app 应用" : "命令名称或程序路径"}
            onChange={(event) => change(draft.kind === "app" ? { app: event.target.value } : { command: event.target.value })} />
          <button type="button" className="btn-secondary" title="选择文件" aria-label="选择文件" onClick={() => void browse()}><FolderIcon className="w-4 h-4" /></button>
        </div></label>}
      {draft.kind === "cli" && <label>参数 <span className="mux-launch-hint">每行一个，可留空</span>
        <textarea className="mux-dialog-input mux-launch-arguments" rows={3} value={draft.args} onChange={(event) => change({ args: event.target.value })} />
      </label>}
    </>}
  </fieldset>;
}
