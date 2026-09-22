import { open } from "@tauri-apps/plugin-dialog";
import type { AgentLaunchInfo, LaunchTarget } from "../lib/agentLaunch";
import { formatError } from "../lib/format";
import { Switch } from "./ui";
import { DialogDisclosure } from "./DialogDisclosure";
import { FolderIcon, LanguageIcon, PlusIcon, TerminalIcon, TrashIcon } from "./icons";

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
  environment: { name: string; value: string }[];
}

export function createAgentLaunchDraft(info: AgentLaunchInfo): AgentLaunchDraft {
  const target = info.configured_target ?? info.resolved_target;
  return { kind: target?.kind ?? info.kind ?? "app", app: target?.kind === "app" ? target.path : "",
    appArgs: target?.kind === "app" ? (target.args ?? []).join("\n") : "", newInstance: target?.kind === "app" && Boolean(target.new_instance),
    command: target?.kind === "cli" ? target.command : "", args: target?.kind === "cli" ? target.args.join("\n") : "",
    url: target?.kind === "web" ? target.url : "", resetDefault: false, directory: info.default_directory ?? "",
    environment: Object.entries(target && target.kind !== "web" ? target.env ?? {} : {}).map(([name, value]) => ({ name, value })) };
}

export function agentLaunchDraftTarget(draft: AgentLaunchDraft, info?: AgentLaunchInfo): LaunchTarget | null {
  if (info && !draft.resetDefault && JSON.stringify(agentLaunchDraftTarget(draft)) === JSON.stringify(agentLaunchDraftTarget(createAgentLaunchDraft(info)))) return info.configured_target;

  if (draft.resetDefault) return null;
  const env = Object.fromEntries(draft.environment.filter((row) => row.name.trim() || row.value)
    .map((row) => [row.name.trim(), row.value]).sort(([a], [b]) => a.localeCompare(b)));
  if (draft.kind === "app") return { kind: "app", path: draft.app.trim(), args: draft.appArgs.split(/\r?\n/).filter(Boolean), new_instance: draft.newInstance, env };
  if (draft.kind === "web") return { kind: "web", url: draft.url.trim() };
  return { kind: "cli", command: draft.command.trim(), args: draft.args.split("\n").filter(Boolean), env };
}

function environmentError(draft: AgentLaunchDraft): string | null {
  if (draft.resetDefault || draft.kind === "web") return null;
  const rows = draft.environment.filter((row) => row.name.trim() || row.value);
  if (rows.length > 64) return "环境变量最多 64 项";
  const names = new Set<string>();
  for (const row of rows) {
    const name = row.name.trim();
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name) || name.length > 256) return "请输入有效变量名，例如 NODE_USE_SYSTEM_CA";
    if (names.has(name)) return "环境变量名不能重复";
    names.add(name);
    if (row.value.includes("\0") || new TextEncoder().encode(row.value).length > 8192) return "环境变量值无效或过长";
  }
  return null;
}

export function agentLaunchDraftValid(draft: AgentLaunchDraft) {
  if (environmentError(draft)) return false;
  return draft.resetDefault || Boolean(draft.kind === "app" ? draft.app.trim() : draft.kind === "cli" ? draft.command.trim() : draft.url.trim());
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
        onClick={() => onChange({ ...createAgentLaunchDraft(info), resetDefault: true, directory: "" })}>恢复默认</button>}
    </div>
    {draft.resetDefault ? <p className="mux-launch-hint">保存后使用 Agent 默认启动方式</p> : <>
      {draft.kind === "web" ? <label>网址<input className="mux-dialog-input" value={draft.url} placeholder="https://" onChange={(event) => change({ url: event.target.value })} /></label>
        : <label>{draft.kind === "app" ? "应用" : "可执行程序"}<div className="mux-launch-path-input">
          <input className="mux-dialog-input" value={draft.kind === "app" ? draft.app : draft.command} placeholder={draft.kind === "app" ? "选择 .app 应用" : "命令名称或程序路径"}
            onChange={(event) => change(draft.kind === "app" ? { app: event.target.value } : { command: event.target.value })} />
          <button type="button" className="btn-secondary" title="选择文件" aria-label="选择文件" onClick={() => void browse()}><FolderIcon className="w-4 h-4" /></button>
        </div></label>}
      {draft.kind === "cli" && <label>默认工作目录<div className="mux-launch-path-input">
        <input className="mux-dialog-input" value={draft.directory} placeholder="留空沿用上次目录" onChange={(event) => change({ directory: event.target.value })} />
        <button type="button" className="btn-secondary" aria-label="选择默认工作目录" title="选择文件夹" onClick={() => void browseDirectory()}><FolderIcon className="w-4 h-4" /></button>
      </div></label>}
      {(draft.kind === "cli" || draft.kind === "app") && <DialogDisclosure title="高级选项"
        summary={[draft.kind === "app" ? draft.appArgs : draft.args, draft.environment.length > 0 && "环境变量", draft.newInstance && draft.kind === "app" && "新实例"].filter(Boolean).length > 0 ? "已配置" : "参数、环境变量"}
        invalid={Boolean(environmentError(draft))}>
      <label><span className="mux-launch-label-line">启动参数 <span className="mux-launch-hint">每行一个参数</span></span>
        <textarea className="mux-dialog-input mux-launch-arguments" rows={2} value={draft.kind === "app" ? draft.appArgs : draft.args}
          onChange={(event) => change(draft.kind === "app" ? { appArgs: event.target.value } : { args: event.target.value })} />
      </label>
      {(draft.kind === "cli" || draft.kind === "app") && <div className="mux-launch-environment">
        <div className="mux-launch-environment-heading"><span>环境变量</span>
          <button type="button" className="btn-ghost" disabled={draft.environment.length >= 64}
            onClick={() => change({ environment: [...draft.environment, { name: "", value: "" }] })}>
            <PlusIcon className="w-3.5 h-3.5" />添加</button>
        </div>
        {draft.environment.map((row, index) => <div className="mux-launch-env-row" key={index}>
          <input className="mux-dialog-input" aria-label={`环境变量 ${index + 1} 名称`} placeholder="NODE_USE_SYSTEM_CA" spellCheck={false} value={row.name}
            onChange={(event) => change({ environment: draft.environment.map((item, position) => position === index ? { ...item, name: event.target.value } : item) })} />
          <span aria-hidden="true">=</span>
          <input className="mux-dialog-input" aria-label={`环境变量 ${index + 1} 值`} placeholder="1" spellCheck={false} value={row.value}
            onChange={(event) => change({ environment: draft.environment.map((item, position) => position === index ? { ...item, value: event.target.value } : item) })} />
          <button type="button" className="mux-launch-reset" aria-label={`删除环境变量 ${index + 1}`}
            onClick={() => change({ environment: draft.environment.filter((_, position) => position !== index) })}><TrashIcon className="w-4 h-4" /></button>
        </div>)}
        {environmentError(draft) && <p className="mux-launch-error" role="alert">{environmentError(draft)}</p>}
      </div>}
      {draft.kind === "app" && <div className="mux-launch-instance-option">
        <div><span>新实例启动</span><p className="mux-launch-hint">另开进程接收参数，需应用支持。</p></div>
        <Switch ariaLabel="新实例启动" checked={draft.newInstance} disabled={disabled} onChange={(value) => change({ newInstance: value })} />
      </div>}
      </DialogDisclosure>}
    </>}
  </fieldset>;
}
