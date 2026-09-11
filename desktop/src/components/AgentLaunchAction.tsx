import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getAgentLaunchInfo, type AgentLaunchInfo } from "../lib/agentLaunch";
import { formatError } from "../lib/format";
import { ChevronDownIcon, DownloadIcon, ExternalLinkIcon, FolderIcon, EditIcon, PlayIcon, RefreshIcon } from "./icons";
import { useAgentLauncher } from "../lib/agentLauncherContext";
import { useToast } from "./Toast";
import installLinks from "../../../data/agent-install-links.json";

export function AgentLaunchAction({ agentId, contextMenu = false, showLabel = false, onChosen, onConfigure }: { agentId: string; contextMenu?: boolean; showLabel?: boolean; onChosen?(): void; onConfigure?(): void }) {
  const launcher = useAgentLauncher();
  const [info, setInfo] = useState<AgentLaunchInfo | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [error, setError] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const { show } = useToast();
  useEffect(() => {
    let active = true;
    const refresh = () => {
      void getAgentLaunchInfo(agentId).then((value) => { if (active) { setInfo(value); setError(false); } })
        .catch(() => { if (active) setError(true); });
    };
    refresh(); window.addEventListener("focus", refresh);
    return () => { active = false; window.removeEventListener("focus", refresh); };
  }, [agentId, launcher.revision]);
  useEffect(() => {
    if (!expanded || contextMenu) return;
    const outside = (event: PointerEvent) => { if (!root.current?.contains(event.target as Node)) setExpanded(false); };
    document.addEventListener("pointerdown", outside);
    root.current?.querySelector<HTMLButtonElement>('[role="menuitem"]')?.focus();
    return () => document.removeEventListener("pointerdown", outside);
  }, [expanded, contextMenu]);
  const busy = launcher.busyId !== null;
  const missing = info && !info.available && info.kind && info.install_url && !info.configured_target;
  const label = !info ? error ? "重试" : "检测中…" : !info.supported ? "暂不支持" : missing ? "下载安装"
    : !info.available ? "设置启动方式" : info.host_name ? `打开 ${info.host_name}` : "打开 Agent";
  const pending = launcher.busyId === agentId || (!info && !error);
  const actionLabel = launcher.busyId === agentId ? "正在打开…" : label;
  const ready = Boolean(info?.available);
  const installUrl = info?.install_url ?? (installLinks as Record<string, { url: string }>)[agentId]?.url;
  const installationStatus = !info ? error ? "状态未知" : "检测中…" : ready ? "启动入口可用" : "未检测到启动程序";
  const canChooseDirectory = info?.kind === "cli" && info.available;
  const showSetup = contextMenu || !onConfigure;
  const hasMenu = Boolean(canChooseDirectory || showSetup);
  const shortLabel = pending ? "稍候" : !info && error ? "重试" : info?.supported === false ? "不支持" : ready ? info?.host_name ?? "运行" : missing ? "安装" : "设置";
  const choose = (action: () => void) => { setExpanded(false); onChosen?.(); action(); };
  const configure = () => { if (onConfigure) onConfigure(); else void launcher.configure(agentId); };
  const launch = () => choose(() => {
    if (info && !info.available && !missing && info.supported && onConfigure) onConfigure();
    else void launcher.launch(agentId);
  });
  const openInstallation = () => {
    if (installUrl) void openUrl(installUrl).catch((failure) => show({ kind: "error", msg: formatError(failure) }));
  };
  const menu = <>
    {contextMenu && <button type="button" role="menuitem" disabled={busy || (!info && !error) || info?.supported === false} onClick={launch}>
      {missing ? <DownloadIcon className="w-4 h-4" /> : <ExternalLinkIcon className="w-4 h-4" />}{label}</button>}
    {canChooseDirectory && <button type="button" role="menuitem" disabled={busy} title={info?.directory ?? undefined}
      onClick={() => choose(() => { void launcher.launch(agentId, true); })}><FolderIcon className="w-4 h-4" />在其他目录打开…</button>}
    {contextMenu && installUrl && !missing && <button type="button" role="menuitem" disabled={busy} onClick={() => choose(openInstallation)}>
      <DownloadIcon className="w-4 h-4" />安装文档</button>}
    {showSetup && <button type="button" role="menuitem" disabled={busy} onClick={() => choose(configure)}>
      <EditIcon className="w-4 h-4" />启动设置…</button>}
  </>;
  return <>
    {!contextMenu && installUrl && <button type="button" className="mux-launch-install btn-secondary" onClick={openInstallation}
      title={`安装文档 · ${installationStatus}`} aria-label={`${info?.name ?? agentId} 安装文档，${installationStatus}`}>
      <DownloadIcon className="w-3.5 h-3.5" />安装文档
      <span className="mux-launch-install-status" data-ready={ready || undefined} aria-hidden="true" />
    </button>}
    <div ref={root} className={contextMenu ? "mux-launch-menu-content" : "mux-launch-action"}
    data-ready={ready || undefined} data-labeled={showLabel || undefined} data-split={hasMenu} aria-busy={pending} onKeyDown={(event) => {
    if (contextMenu) return; // The portal menu owns focus, Escape and Tab.
    if (event.key === "Escape") { event.stopPropagation(); setExpanded(false); trigger.current?.focus(); }
    if ((expanded || contextMenu) && ["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const items = Array.from(root.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') ?? []);
      const current = items.indexOf(document.activeElement as HTMLButtonElement);
      const next = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : (current + (event.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
      items[next]?.focus();
    }
  }}>
    {contextMenu ? menu : <>
      <button type="button" className="mux-launch-primary" disabled={busy || (!info && !error) || info?.supported === false} onClick={launch}
        aria-label={actionLabel}
        title={`${actionLabel}${info?.available ? ` · ${info.host_name ?? info.name}` : ""}${info?.directory && info.kind === "cli" ? `\n${info.directory}` : ""}`}>
        <span className="mux-launch-symbol" data-spinning={pending || undefined} aria-hidden="true">
          {pending || (!info && error) ? <RefreshIcon /> : missing ? <DownloadIcon /> : !ready ? <EditIcon /> : <PlayIcon />}
        </span>
        {showLabel && <span className="mux-launch-caption" aria-hidden="true">{shortLabel}</span>}
        {ready && !pending && !showLabel && <i className="mux-launch-ready-dot" aria-hidden="true" />}
      </button>
      {hasMenu && <button ref={trigger} type="button" className="mux-launch-more" aria-label="Agent 启动选项" aria-haspopup="menu" aria-expanded={expanded}
        disabled={busy} onClick={() => setExpanded((value) => !value)}><ChevronDownIcon className="w-3.5 h-3.5" /></button>}
      {hasMenu && expanded && <div className="mux-launch-menu" role="menu" aria-label="Agent 启动选项">{menu}</div>}
    </>}
    </div>
  </>;
}
