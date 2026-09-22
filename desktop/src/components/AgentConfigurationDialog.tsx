import { RESOURCE_CATEGORIES, ResourceIcon } from "./resourcePresentation";
import { Fragment, useEffect, useId, useRef, useState, type ReactNode } from "react";
import type {
  AgentConfigurationPatch,
  AgentInfo,
  ApiKeyDelivery,
  AssetOperationPlan,
  ModelAgentView,
} from "../lib/types";
import {
  cancelOperation,
  commitOperation,
  planOperation,
  setAgentCredentialDelivery,
} from "../lib/api";
import { formatError } from "../lib/format";
import { DialogShell } from "./DialogShell";
import { AssetOperationReviewDialog } from "./AssetOperationReviewDialog";
import { ExternalLinkIcon, KeyIcon, PlusIcon, TrashIcon } from "./icons";
import { useToast } from "./Toast";
import { configureAgentLaunch, getAgentLaunchInfo, type AgentLaunchInfo } from "../lib/agentLaunch";
import { AgentLaunchFields, agentLaunchDraftChanged, agentLaunchDraftTarget, agentLaunchDraftValid, createAgentLaunchDraft, type AgentLaunchDraft } from "./AgentLaunchFields";
import { AgentGlyph } from "./brandIcons";
import { FormSelect } from "./FormSelect";
import "./AgentLaunch.css";
import { homeDir } from "@tauri-apps/api/path";
import { openPath } from "@tauri-apps/plugin-opener";
import { preferredFileEditor } from "./FileEditorSelect";

function deliveryLabel(value: ApiKeyDelivery) {
  if (value === "env") return "环境变量";
  if (value === "command") return "命令";
  if (value === "agent-store") return "Agent 凭据库";
  if (value === "plaintext") return "明文配置";
  return "自动适配";
}

function resolvedAgentDelivery(agent: ModelAgentView): ApiKeyDelivery {
  const available = agent.available_deliveries ?? [];
  const stored = agent.default_delivery ?? "plaintext";
  if (available.includes(stored)) return stored;
  if (available.includes("plaintext")) return "plaintext";
  return available[0] ?? "auto";
}

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
  const [section, setSection] = useState<"launch" | "paths">(initialSection);
  const sectionId = useId();
  const sectionButtons = useRef<Array<HTMLButtonElement | null>>([]);
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
  const hasDelivery = modelAgent?.mode === "managed" && (modelAgent.available_deliveries?.length ?? 0) > 0;
  const [delivery, setDelivery] = useState<ApiKeyDelivery>(() => modelAgent ? resolvedAgentDelivery(modelAgent) : "auto");
  const [savedDelivery, setSavedDelivery] = useState(delivery);
  const [confirmingPlaintext, setConfirmingPlaintext] = useState(false);
  const plaintextApproved = useRef(false);
  const deliveryChanged = hasDelivery && delivery !== savedDelivery;
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
  const canSubmit = !busy && !browsing && !launchLoading && (configurationChanged || launchChanged || deliveryChanged)
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
    if (initialSection !== "launch" || section !== "launch" || launchLoading || didFocusLaunch.current) return;
    didFocusLaunch.current = true;
    launchSection.current?.scrollIntoView({ block: "nearest" });
    launchSection.current?.querySelector<HTMLInputElement>("input:not(:disabled)")?.focus({ preventScroll: true });
  }, [initialSection, section, launchLoading]);

  const saveLaunch = async () => {
    if (!launchChanged || !launchDraft) return;
    const info = await configureAgentLaunch(agent.id, agentLaunchDraftTarget(launchDraft, launchInfo ?? undefined), launchDraft.directory.trim());
    setLaunchInfo(info); setLaunchDraft(createAgentLaunchDraft(info));
    onLaunchSaved?.();
  };

  const savePreferences = async () => {
    if (deliveryChanged) {
      await setAgentCredentialDelivery(agent.id, delivery, plaintextApproved.current);
      setSavedDelivery(delivery);
    }
    await saveLaunch();
  };

  const save = async () => {
    if (!canSubmit) return;
    if (deliveryChanged && delivery === "plaintext" && !plaintextApproved.current) {
      setConfirmingPlaintext(true);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      if (!configurationChanged) {
        await savePreferences();
        await onSaved();
        toast.show({ kind: "success", msg: `${agent.name} 配置已更新。` });
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
      await savePreferences();
      launchCommitted = true;
      await onSaved();
      toast.show({ kind: "success", msg: `${agent.name} 配置已更新。` });
      onClose();
    } catch (commitError) {
      setError(configurationCommitted
        ? `${launchCommitted ? "配置已保存，刷新失败" : "配置路径已保存，凭据或启动设置尚未全部保存，可重试"}：${formatError(commitError)}`
        : formatError(commitError));
      if (configurationCommitted && !launchCommitted) {
        // Keep unsaved preferences so retry does not repeat committed changes.
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
      plaintextApproved.current = false;
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

  if (confirmingPlaintext) {
    return (
      <DialogShell kind="review" size="sm" className="mux-plaintext-confirmation"
        title="明文写入 API Key" subtitle={agent.name}
        onClose={() => setConfirmingPlaintext(false)}
        footerEnd={<>
          <button type="button" className="btn-secondary" onClick={() => setConfirmingPlaintext(false)}>返回编辑</button>
          <button type="button" className="btn-danger" onClick={() => {
            plaintextApproved.current = true;
            setConfirmingPlaintext(false);
            void save();
          }}>确认明文写入</button>
        </>}
      >
        <div className="mux-plaintext-confirmation-body">
          <strong>将把 Provider API Key 明文写入 {agent.name} 配置</strong>
          <code>{modelPaths[0]?.trim() || modelAgent?.config_path}</code>
          <span>仅对该 Agent 生效，文件权限将收紧为 0600。之后添加的 Model 都按此策略写入。</span>
        </div>
      </DialogShell>
    );
  }

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
      size="wide"
      title={agent.name}
      leading={<AgentGlyph id={agent.id} name={agent.name} size={32} />}
      status={<div className="mux-agent-config-tabs" role="tablist" aria-label="配置分类">
        {([['launch', '启动'], ['paths', '配置文件']] as const).map(([value, label], index) => <button
          key={value} ref={(node) => { sectionButtons.current[index] = node; }} type="button" role="tab"
          id={`${sectionId}-${value}-tab`} aria-controls={`${sectionId}-${value}-panel`} aria-selected={section === value}
          tabIndex={section === value ? 0 : -1} disabled={busy || browsing} onClick={() => setSection(value)}
          onKeyDown={(event) => {
            if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
            event.preventDefault();
            const next = event.key === "Home" ? 0 : event.key === "End" ? 1 : 1 - index;
            setSection(next === 0 ? "launch" : "paths"); sectionButtons.current[next]?.focus();
          }}>
          {label}{(value === "launch" ? launchChanged : configurationChanged || deliveryChanged) && <i aria-label="未保存" />}
        </button>)}
      </div>}
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
      <div hidden={section !== "paths"} role="tabpanel" id={`${sectionId}-paths-panel`} aria-labelledby={`${sectionId}-paths-tab`}>
      <fieldset className="mux-agent-config-form mux-agent-config-fields" disabled={busy || browsing}>
        {RESOURCE_CATEGORIES.map((resource) => <Fragment key={resource.id}>
          {resource.id === "models" && <>
        {modelPaths.length > 0 ? modelPaths.map((path, index) => (
          <ConfigField
            key={index}
            icon={<ResourceIcon domain="model" />}
            label={modelPaths.length > 1 ? `${resource.label} ${index + 1}` : resource.label}
            value={path}
            openKind="file"
            onChange={(value) => updateModelPath(index, value)}
          />
        )) : (
          <ConfigField
            icon={<ResourceIcon domain="model" />}
            label={resource.label}
            value="未接入"
            disabled
          />
        )}
          </>}
          {resource.id === "mcps" && <>
        {hasMcp ? (
          <div className="mux-agent-config-mcp">
            <ConfigField
              icon={<ResourceIcon domain="mcp" />}
              label={resource.label}
              openKind="file"
              value={mcpPath}
              onChange={setMcpPath}
            />
            <ConfigField
              icon={null}
              label="配置键"
              value={mcpKey}
              onChange={setMcpKey}
            />
          </div>
        ) : (
          <ConfigField
            icon={<ResourceIcon domain="mcp" />}
            label={resource.label}
            value="未接入"
            disabled
          />
        )}
          </>}
          {resource.id === "skills" && <>
        {skillsPaths.length > 0 ? skillsPaths.map((path, index) => (
          <ConfigField
            key={index}
            icon={<ResourceIcon domain="skill" />}
            label={skillsPaths.length > 1 ? `${resource.label} ${index + 1}` : resource.label}
            value={path}
            openKind="folder"
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
            icon={<ResourceIcon domain="skill" />}
            label={resource.label}
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
          </>}
        </Fragment>)}
      </fieldset>
      {hasDelivery && (
        <section className="mux-agent-config-credential" aria-label="凭据设置">
          <div className="mux-agent-config-credential-row">
            <span className="mux-agent-field-caption"><KeyIcon className="w-4 h-4" />凭据方式</span>
            <FormSelect ariaLabel="凭据方式" value={delivery} disabled={busy || browsing}
              options={(modelAgent?.available_deliveries ?? []).map((value) => ({ value, label: deliveryLabel(value) }))}
              onChange={(value) => { setDelivery(value as ApiKeyDelivery); plaintextApproved.current = false; }} />
          </div>
        </section>
      )}
      </div>
      <div hidden={section !== "launch"} role="tabpanel" id={`${sectionId}-launch-panel`} aria-labelledby={`${sectionId}-launch-tab`}>
      <section ref={launchSection} className="mux-agent-config-launch" aria-label="启动设置">
        {launchLoading ? <p className="mux-launch-hint" role="status">读取中…</p>
          : launchInfo && launchDraft ? <AgentLaunchFields info={launchInfo} draft={launchDraft} disabled={busy || browsing}
            onChange={setLaunchDraft} onBusyChange={setBrowsing} onError={setLaunchError} /> : null}
        {launchError && <div className="mux-launch-error" role="alert">{launchError}
          {!launchInfo && <button type="button" className="btn-ghost" disabled={busy || browsing || launchLoading} onClick={() => setLaunchRequest((value) => value + 1)}>重试</button>}
        </div>}
      </section>
      </div>
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
  openKind,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  disabled?: boolean;
  onChange?(value: string): void;
  action?: ReactNode;
  openKind?: "file" | "folder";
}) {
  const toast = useToast();
  const openLocation = async () => {
    try {
      const home = (await homeDir()).replace(/\/$/, "");
      const path = value.trim();
      const absolute = path === "~" ? home : path.startsWith("~/") ? `${home}/${path.slice(2)}` : path;
      const editor = openKind === "file" ? preferredFileEditor() : undefined;
      if (editor) await openPath(absolute, editor); else await openPath(absolute);
    } catch (error) { toast.show({ kind: "error", msg: `无法打开：${formatError(error)}` }); }
  };
  if (disabled && value === "未接入") return <div className="mux-agent-config-unavailable">
    <span className="mux-agent-field-caption">{icon}{label}</span><span>未接入</span>
  </div>;
  return (
    <label data-path-field={openKind || undefined} className="mux-agent-config-field" data-disabled={disabled || undefined}>
      <span className="mux-agent-field-caption">{icon}{label}</span>
      <span className="mux-agent-field-control">
      <input
        className="mux-model-field"
        value={value}
        disabled={disabled}
        spellCheck={false}
        onChange={(event) => onChange?.(event.target.value)}
      />
      {(openKind || action) && <span className="mux-agent-config-field-actions">
        {openKind && <button type="button" className="mux-agent-config-remove" disabled={disabled || !value.trim()}
          title={openKind === "file" ? "使用编辑器打开" : "打开文件夹"} aria-label={`打开 ${label}`}
          onClick={(event) => { event.preventDefault(); void openLocation(); }}><ExternalLinkIcon className="w-4 h-4" /></button>}
        {action}
      </span>}
      </span>
    </label>
  );
}
