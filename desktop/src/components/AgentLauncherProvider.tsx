import { useCallback, useRef, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getAgentLaunchInfo, launchAgent, type AgentLaunchInfo } from "../lib/agentLaunch";
import { formatError } from "../lib/format";
import { AgentLaunchSettings } from "./AgentLaunchSettings";
import { useToast } from "./Toast";
import "./AgentLaunch.css";
import { AgentLauncherContext } from "../lib/agentLauncherContext";

export function AgentLauncherProvider({ children }: { children: ReactNode }) {
  const [busyId, setBusyId] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const [editing, setEditing] = useState<AgentLaunchInfo | null>(null);
  const running = useRef(false);
  const { show } = useToast();
  const refresh = useCallback(() => setRevision((value) => value + 1), []);
  const configure = useCallback(async (id: string) => {
    if (running.current) return;
    running.current = true; setBusyId(id);
    try { setEditing(await getAgentLaunchInfo(id)); }
    catch (error) { show({ kind: "error", msg: `无法读取启动方式：${formatError(error)}` }); }
    finally { running.current = false; setBusyId(null); }
  }, [show]);
  const launch = useCallback(async (id: string, chooseDirectory = false) => {
    if (running.current) return;
    running.current = true; setBusyId(id);
    try {
      const info = await getAgentLaunchInfo(id);
      if (!info.supported) throw new Error("当前平台暂不支持启动 Agent");
      if (!info.available) {
        if (info.kind && info.install_url && !info.configured_target) await openUrl(info.install_url);
        else setEditing(info);
        return;
      }
      let directory = info.directory;
      if (info.kind === "cli" && (chooseDirectory || !directory || !info.directory_exists)) {
        directory = await open({ title: `${info.name} · 选择工作目录`, directory: true, multiple: false,
          ...(info.directory_exists && directory ? { defaultPath: directory } : {}) });
        if (!directory) return;
      }
      const result = await launchAgent(id, directory);
      setRevision((value) => value + 1);
      show({ kind: "success", msg: info.kind === "cli" ? `已向终端发送 ${info.name} 启动请求` : `已打开 ${info.host_name ?? info.name}` });
      if (!result.directory_saved) show({ kind: "error", msg: "已发送启动请求，但未能记住工作目录" });
    } catch (error) { show({ kind: "error", msg: `无法打开 Agent：${formatError(error)}` }); }
    finally { running.current = false; setBusyId(null); }
  }, [show]);
  return <AgentLauncherContext.Provider value={{ busyId, revision, launch, configure, refresh }}>
    {children}
    {editing && <AgentLaunchSettings key={editing.agent_id} info={editing} onClose={() => setEditing(null)} onSaved={() => setRevision((value) => value + 1)} />}
  </AgentLauncherContext.Provider>;
}
