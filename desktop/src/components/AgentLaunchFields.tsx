import { open } from "@tauri-apps/plugin-dialog";
import type { AgentLaunchInfo, LaunchTarget } from "../lib/agentLaunch";
import { formatError } from "../lib/format";
import { formatLaunchEnvironment, parseLaunchEnvironment } from "../lib/launchEnvironment";
import { Switch } from "./ui";
import { DialogDisclosure } from "./DialogDisclosure";
import { FolderIcon } from "./icons";

export interface AgentLaunchDraft {
  kind: LaunchTarget["kind"];
  app: string;
  appArgs: string;
  newInstance: boolean;
  command: string;
  args: string;
  url: string;
  resetDefault: boolean;
  directory: string;
  environment: string;
}

function fixedKind(info: AgentLaunchInfo): "app" | "cli" {
  return info.kind === "cli" ? "cli" : "app";
}

export function createAgentLaunchDraft(info: AgentLaunchInfo): AgentLaunchDraft {
  const kind = fixedKind(info);
  const target = info.configured_target?.kind === kind ? info.configured_target : info.resolved_target?.kind === kind ? info.resolved_target : null;
  return { kind, app: target?.kind === "app" ? target.path : "",
    appArgs: target?.kind === "app" ? (target.args ?? []).join("\n") : "", newInstance: target?.kind === "app" && Boolean(target.new_instance),
    command: target?.kind === "cli" ? target.command : "", args: target?.kind === "cli" ? target.args.join("\n") : "",
    url: "", resetDefault: false, directory: info.default_directory ?? "",
    environment: formatLaunchEnvironment(target?.env) };
}

export function agentLaunchDraftTarget(draft: AgentLaunchDraft, info?: AgentLaunchInfo): LaunchTarget | null {
  if (info && !draft.resetDefault && JSON.stringify(agentLaunchDraftTarget(draft)) === JSON.stringify(agentLaunchDraftTarget(createAgentLaunchDraft(info)))) return info.configured_target;

  if (draft.resetDefault) return null;
  const env = parseLaunchEnvironment(draft.environment).env;
  if (draft.kind === "app") return { kind: "app", path: draft.app.trim(), args: draft.appArgs.split(/\r?\n/).filter(Boolean), new_instance: draft.newInstance, env };
  return { kind: "cli", command: draft.command.trim(), args: draft.args.split("\n").filter(Boolean), env };
}

function environmentError(draft: AgentLaunchDraft): string | null {
  if (draft.resetDefault) return null;
  return parseLaunchEnvironment(draft.environment).error;
}

export function agentLaunchDraftValid(draft: AgentLaunchDraft) {
  if (environmentError(draft)) return false;
  return draft.resetDefault || Boolean(draft.kind === "app" ? draft.app.trim() : draft.command.trim());
}

export function agentLaunchDraftChanged(info: AgentLaunchInfo, draft: AgentLaunchDraft) {
  if (environmentError(draft)) return true;
  if (draft.directory.trim() !== (info.default_directory ?? "")) return true;
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
  const browseDirectory = async () => {
    if (disabled) return;
    onBusyChange(true); onError("");
    try {
      const value = await open({ title: "默认工作目录", directory: true, multiple: false,
        ...(draft.directory || info.directory ? { defaultPath: draft.directory || info.directory! } : {}) });
      if (value) change({ directory: value });
    } catch (error) { onError(formatError(error)); }
    finally { onBusyChange(false); }
  };
  const appLabel = info.category === "plugin" ? "宿主应用" : info.category === "ide" ? "IDE" : "应用";
  return <fieldset className="mux-launch-settings mux-launch-fields" disabled={disabled}>
    <div className="mux-launch-field-modes">
      {(info.configured_target || agentLaunchDraftChanged(info, draft)) && <button type="button" className="mux-launch-reset"
        onClick={() => onChange({ ...createAgentLaunchDraft(info), resetDefault: true, directory: "" })}>恢复默认</button>}
    </div>
    {draft.resetDefault ? <p className="mux-launch-hint">保存后使用 Agent 默认启动方式</p> : <>
      <label>{draft.kind === "app" ? appLabel : "可执行程序"}<div className="mux-launch-path-input">
          <input className="mux-dialog-input" value={draft.kind === "app" ? draft.app : draft.command} placeholder={draft.kind === "app" ? "选择 .app 应用" : "命令名称或程序路径"}
            onChange={(event) => change(draft.kind === "app" ? { app: event.target.value } : { command: event.target.value })} />
          <button type="button" className="btn-secondary" title="选择文件" aria-label="选择文件" onClick={() => void browse()}><FolderIcon className="w-4 h-4" /></button>
        </div></label>
      {draft.kind === "cli" && <label>默认工作目录<div className="mux-launch-path-input">
        <input className="mux-dialog-input" value={draft.directory} placeholder="留空沿用上次目录" onChange={(event) => change({ directory: event.target.value })} />
        <button type="button" className="btn-secondary" aria-label="选择默认工作目录" title="选择文件夹" onClick={() => void browseDirectory()}><FolderIcon className="w-4 h-4" /></button>
      </div></label>}
      {(draft.kind === "cli" || draft.kind === "app") && <DialogDisclosure title="高级选项"
        summary={[draft.kind === "app" ? draft.appArgs : draft.args, draft.environment.trim() && "环境变量", draft.newInstance && draft.kind === "app" && "新实例"].filter(Boolean).length > 0 ? "已配置" : "参数、环境变量"}
        invalid={Boolean(environmentError(draft))}>
      <label><span className="mux-launch-label-line">启动参数 <span className="mux-launch-hint">每行一个参数</span></span>
        <textarea className="mux-dialog-input mux-launch-arguments" rows={2} value={draft.kind === "app" ? draft.appArgs : draft.args}
          onChange={(event) => change(draft.kind === "app" ? { appArgs: event.target.value } : { args: event.target.value })} />
      </label>
      <label><span className="mux-launch-label-line">环境变量 <span className="mux-launch-hint">每行 NAME=VALUE，可直接粘贴</span></span>
        <textarea className="mux-dialog-input mux-launch-arguments mux-launch-environment-block" rows={6} spellCheck={false} aria-label="环境变量"
          placeholder={"HTTP_PROXY=http://127.0.0.1:6789\nNO_PROXY=localhost,127.0.0.1"} value={draft.environment}
          onChange={(event) => change({ environment: event.target.value })} />
        {environmentError(draft) && <p className="mux-launch-error" role="alert">{environmentError(draft)}</p>}
      </label>
      {draft.kind === "app" && <div className="mux-launch-instance-option">
        <div><span>新实例启动</span><p className="mux-launch-hint">另开进程接收参数，需应用支持。</p></div>
        <Switch ariaLabel="新实例启动" checked={draft.newInstance} disabled={disabled} onChange={(value) => change({ newInstance: value })} />
      </div>}
      </DialogDisclosure>}
    </>}
  </fieldset>;
}
