import { useEffect, useRef, useState, type ReactNode } from "react";
import type {
  AgentConfigurationPatch,
  AgentInfo,
  AssetOperationPlan,
  ModelAgentView,
} from "../lib/types";
import {
  cancelOperation,
  commitOperation,
  planOperation,
} from "../lib/api";
import { formatError } from "../lib/format";
import { DialogShell } from "./DialogShell";
import { AssetOperationReviewDialog } from "./AssetOperationReviewDialog";
import { LayersIcon, PackageIcon, PlusIcon, SparklesIcon, TrashIcon } from "./icons";
import { useToast } from "./Toast";
import { configureAgentLaunch, getAgentLaunchInfo, type AgentLaunchInfo } from "../lib/agentLaunch";
import { AgentLaunchFields, agentLaunchDraftChanged, agentLaunchDraftTarget, agentLaunchDraftValid, createAgentLaunchDraft, type AgentLaunchDraft } from "./AgentLaunchFields";
import "./AgentLaunch.css";

export function AgentConfigurationDialog({
  agent,
  modelAgent,
  onClose,
  onSaved,
  onLaunchSaved,
  initialSection = "paths",
}: {
  agent: AgentInfo;
  modelAgent: ModelAgentView | null;
  onClose(): void;
  onSaved(): Promise<unknown> | unknown;
  onLaunchSaved?(): void;
  initialSection?: "paths" | "launch";
}) {
  const initialModelPaths = modelAgent?.config_paths?.length
    ? modelAgent.config_paths
    : modelAgent?.config_path
      ? [modelAgent.config_path]
      : [];
  const [mcpPath, setMcpPath] = useState(agent.global ?? "");
  const [mcpKey, setMcpKey] = useState(agent.key);
  const [modelPaths, setModelPaths] = useState(initialModelPaths);
  const [skillsPaths, setSkillsPaths] = useState(
    agent.skills_global_dirs?.length
      ? agent.skills_global_dirs
      : agent.skills_global_dir ? [agent.skills_global_dir] : [],
  );
  const [busy, setBusy] = useState(false);
  const [plan, setPlan] = useState<AssetOperationPlan | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [launchInfo, setLaunchInfo] = useState<AgentLaunchInfo | null>(null);
  const [launchDraft, setLaunchDraft] = useState<AgentLaunchDraft | null>(null);
  const [launchLoading, setLaunchLoading] = useState(true);
  const [launchError, setLaunchError] = useState("");
  const [launchRequest, setLaunchRequest] = useState(0);
  const [browsing, setBrowsing] = useState(false);
  const reviewedConfiguration = useRef<string | null>(null);
  const launchSection = useRef<HTMLElement>(null);
  const didFocusLaunch = useRef(false);
  const toast = useToast();
  const hasMcp = agent.has_global;
  const hasModel = modelAgent !== null;
  const hasSkills = agent.skills_global_dir !== null;

  const configurationPatch = (): AgentConfigurationPatch => ({
    ...(hasMcp ? { mcp: { path: mcpPath.trim(), key: mcpKey.trim() } } : {}),
    ...(hasModel ? { model: { paths: modelPaths.map((path) => path.trim()) } } : {}),
    ...(hasSkills ? { skill: { global_dir: skillsPaths[0]?.trim() ?? "", alias_dirs: skillsPaths.slice(1).map((path) => path.trim()) } } : {}),
  });
  const [savedConfiguration, setSavedConfiguration] = useState(() => JSON.stringify(configurationPatch()));
  const configurationChanged = JSON.stringify(configurationPatch()) !== savedConfiguration;
  const launchChanged = Boolean(launchInfo && launchDraft && agentLaunchDraftChanged(launchInfo, launchDraft));
  const configurationValid = (!hasMcp || (mcpPath.trim().length > 0 && mcpKey.trim().length > 0))
    && (!hasModel || (modelPaths.length > 0
      && modelPaths.every((path) => path.trim().length > 0)))
    && (!hasSkills || (skillsPaths.length > 0
      && skillsPaths.every((path) => path.trim().length > 0)));
  const canSubmit = !busy && !browsing && !launchLoading && (configurationChanged || launchChanged)
    && (!configurationChanged || configurationValid) && (!launchChanged || launchDraft && agentLaunchDraftValid(launchDraft));

  useEffect(() => {
    let active = true;
    setLaunchLoading(true); setLaunchError("");
    void getAgentLaunchInfo(agent.id).then((info) => {
      if (!active) return;
      setLaunchInfo(info); setLaunchDraft(createAgentLaunchDraft(info));
    }).catch((failure) => { if (active) setLaunchError(formatError(failure)); })
      .finally(() => { if (active) setLaunchLoading(false); });
    return () => { active = false; };
  }, [agent.id, launchRequest]);

  useEffect(() => {
    if (initialSection !== "launch" || launchLoading || didFocusLaunch.current) return;
    didFocusLaunch.current = true;
    launchSection.current?.scrollIntoView({ block: "nearest" });
    launchSection.current?.querySelector<HTMLInputElement>("input:not(:disabled)")?.focus({ preventScroll: true });
  }, [initialSection, launchLoading]);

  const saveLaunch = async () => {
    if (!launchChanged || !launchDraft) return;
    const info = await configureAgentLaunch(agent.id, agentLaunchDraftTarget(launchDraft));
    setLaunchInfo(info); setLaunchDraft(createAgentLaunchDraft(info));
    onLaunchSaved?.();
  };

  const save = async () => {
    if (!canSubmit) return;
    setBusy(true);
    setError(null);
    try {
      if (!configurationChanged) {
        await saveLaunch();
        toast.show({ kind: "success", msg: `${agent.name} 启动设置已更新。` });
        onClose();
        return;
      }
      const patch = configurationPatch();
      const result = await planOperation({
        operation: "update_agent_capabilities",
        request: { agent_id: agent.id, patch },
      });
      if (result.domain !== "asset") {
        throw new Error("Core returned a Skill plan for an Agent configuration request");
      }
      reviewedConfiguration.current = JSON.stringify(patch);
      setPlan(result.plan);
    } catch (error) {
      const message = formatError(error);
      setError(message);
      toast.show({ kind: "error", msg: "无法保存配置：" + message });
    } finally {
      setBusy(false);
    }
  };

  const commit = async () => {
    if (!plan) return;
    setBusy(true);
    setError(null);
    let configurationCommitted = false;
    let launchCommitted = false;
    try {
      const result = await commitOperation({
        domain: "asset",
        request: {
          operation_id: plan.operation_id,
          candidate_hash: plan.candidate_hash,
        },
      });
      if (result.domain !== "asset") {
        throw new Error("Core returned a Skill inventory for an Agent configuration commit");
      }
      if (!result.converged) {
        await onSaved();
        throw new Error("MUX 已保存期望配置，但 Agent 文件尚未完成收敛；请在当前配置位置重试。");
      }
      configurationCommitted = true;
      setSavedConfiguration(reviewedConfiguration.current ?? JSON.stringify(configurationPatch()));
      setPlan(null);
      await saveLaunch();
      launchCommitted = true;
      await onSaved();
      toast.show({ kind: "success", msg: `${agent.name} 配置已更新。` });
      onClose();
    } catch (commitError) {
      setError(configurationCommitted
        ? `${launchCommitted ? "配置已保存，刷新失败" : "配置路径已保存，启动设置未保存，可重试"}：${formatError(commitError)}`
        : formatError(commitError));
      if (configurationCommitted && !launchCommitted) {
        // Refresh saved paths, but keep the launch draft so retry cannot repeat
        // the already committed capability operation.
        try { await onSaved(); } catch { /* The original save error stays visible. */ }
      }
    } finally {
      setBusy(false);
    }
  };

  const cancelPlan = async () => {
    if (!plan) return onClose();
    setBusy(true);
    try {
      await cancelOperation({
        domain: "asset",
        operation_id: plan.operation_id,
      });
      setPlan(null);
      setError(null);
    } catch (cancelError) {
      setError(formatError(cancelError));
    } finally {
      setBusy(false);
    }
  };

  const updateModelPath = (index: number, value: string) => {
    setModelPaths((current) => current.map((path, candidate) => (
      candidate === index ? value : path
    )));
  };

  const updateSkillsPath = (index: number, value: string) => {
    setSkillsPaths((current) => current.map((path, candidate) => (
      candidate === index ? value : path
    )));
  };

  if (plan) {
    return (
      <AssetOperationReviewDialog
        plan={plan}
        busy={busy || browsing}
        error={error}
        agentName={agent.name}
        cancelLabel="返回编辑"
        onCommit={commit}
        onCancel={cancelPlan}
      />
    );
  }

  return (
    <DialogShell
      className="mux-dialog-agent-config"
      kind="editor"
      size="md"
      title="编辑配置"
      subtitle={agent.name}
      busy={busy || browsing}
      onClose={onClose}
      footerStart={configurationChanged ? <span className="mux-agent-config-hint">配置路径变更将显示影响范围</span> : null}
      footerEnd={(
        <>
          <button type="button" className="btn-ghost" disabled={busy || browsing} onClick={onClose}>取消</button>
          <button type="button" className="btn-primary" disabled={!canSubmit} onClick={() => void save()}>
            {busy ? "保存中…" : configurationChanged ? "继续" : "保存"}
          </button>
        </>
      )}
    >
      <fieldset className="mux-agent-config-form mux-agent-config-fields" disabled={busy || browsing}>
        {hasMcp ? (
          <>
            <ConfigField
              icon={<PackageIcon className="w-4 h-4" />}
              label="MCP 文件路径"
              value={mcpPath}
              onChange={setMcpPath}
            />
            <ConfigField
              icon={null}
              label="MCP 配置键"
              value={mcpKey}
              onChange={setMcpKey}
            />
          </>
        ) : (
          <ConfigField
            icon={<PackageIcon className="w-4 h-4" />}
            label="MCP"
            value="未接入"
            disabled
          />
        )}
        {modelPaths.length > 0 ? modelPaths.map((path, index) => (
          <ConfigField
            key={index}
            icon={index === 0 ? <LayersIcon className="w-4 h-4" /> : null}
            label={modelPaths.length > 1 ? `Model ${index + 1}` : "Model"}
            value={path}
            onChange={(value) => updateModelPath(index, value)}
          />
        )) : (
          <ConfigField
            icon={<LayersIcon className="w-4 h-4" />}
            label="Model"
            value="未接入"
            disabled
          />
        )}
        {skillsPaths.length > 0 ? skillsPaths.map((path, index) => (
          <ConfigField
            key={index}
            icon={index === 0 ? <SparklesIcon className="w-4 h-4" /> : null}
            label={skillsPaths.length > 1 ? `Skills ${index + 1}` : "Skills"}
            value={path}
            onChange={(value) => updateSkillsPath(index, value)}
            action={index > 0 ? (
              <button
                type="button"
                className="mux-agent-config-remove"
                aria-label={`移除 Skills 目录 ${index + 1}`}
                onClick={() => setSkillsPaths((current) => current.filter((_, candidate) => candidate !== index))}
              >
                <TrashIcon className="w-4 h-4" />
              </button>
            ) : null}
          />
        )) : (
          <ConfigField
            icon={<SparklesIcon className="w-4 h-4" />}
            label="Skills"
            value="未接入"
            disabled
          />
        )}
        {skillsPaths.length > 0 && skillsPaths.length < 16 && (
          <button
            type="button"
            className="mux-agent-config-add"
            onClick={() => setSkillsPaths((current) => [...current, ""])}
          >
            <PlusIcon className="w-3.5 h-3.5" />添加 Skills 目录
          </button>
        )}
      </fieldset>
      <section ref={launchSection} className="mux-agent-config-launch" aria-label="启动设置">
        <h3>启动方式</h3>
        {launchLoading ? <p className="mux-launch-hint" role="status">读取中…</p>
          : launchInfo && launchDraft ? <AgentLaunchFields info={launchInfo} draft={launchDraft} disabled={busy || browsing}
            onChange={setLaunchDraft} onBusyChange={setBrowsing} onError={setLaunchError} /> : null}
        {launchError && <div className="mux-launch-error" role="alert">{launchError}
          {!launchInfo && <button type="button" className="btn-ghost" disabled={busy || browsing || launchLoading} onClick={() => setLaunchRequest((value) => value + 1)}>重试</button>}
        </div>}
      </section>
      {error && <p className="mux-launch-error" role="alert">{error}</p>}
    </DialogShell>
  );
}

function ConfigField({
  icon,
  label,
  value,
  disabled = false,
  onChange,
  action,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  disabled?: boolean;
  onChange?(value: string): void;
  action?: ReactNode;
}) {
  return (
    <label className="mux-agent-config-field" data-disabled={disabled || undefined}>
      <span className="mux-agent-config-field-icon">{icon}</span>
      <strong>{label}</strong>
      <input
        className="mux-model-field"
        value={value}
        disabled={disabled}
        spellCheck={false}
        onChange={(event) => onChange?.(event.target.value)}
      />
      {action}
    </label>
  );
}
