import { useState, type ReactNode } from "react";
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

export function AgentConfigurationDialog({
  agent,
  modelAgent,
  onClose,
  onSaved,
}: {
  agent: AgentInfo;
  modelAgent: ModelAgentView | null;
  onClose(): void;
  onSaved(): Promise<unknown> | unknown;
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
  const toast = useToast();
  const hasMcp = agent.has_global;
  const hasModel = modelAgent !== null;
  const hasSkills = agent.skills_global_dir !== null;

  const canSubmit = !busy
    && (!hasMcp || (mcpPath.trim().length > 0 && mcpKey.trim().length > 0))
    && (!hasModel || (modelPaths.length > 0
      && modelPaths.every((path) => path.trim().length > 0)))
    && (!hasSkills || (skillsPaths.length > 0
      && skillsPaths.every((path) => path.trim().length > 0)));

  const save = async () => {
    if (!canSubmit) return;
    setBusy(true);
    setError(null);
    try {
      const patch: AgentConfigurationPatch = {};
      if (hasMcp) {
        patch.mcp = { path: mcpPath.trim(), key: mcpKey.trim() };
      }
      if (hasModel) {
        patch.model = { paths: modelPaths.map((path) => path.trim()) };
      }
      if (hasSkills) {
        patch.skill = {
          global_dir: skillsPaths[0].trim(),
          alias_dirs: skillsPaths.slice(1).map((path) => path.trim()),
        };
      }
      const result = await planOperation({
        operation: "update_agent_capabilities",
        request: { agent_id: agent.id, patch },
      });
      if (result.domain !== "asset") {
        throw new Error("Core returned a Skill plan for an Agent configuration request");
      }
      setPlan(result.plan);
    } catch (error) {
      const message = formatError(error);
      setError(message);
      toast.show({ kind: "error", msg: "无法保存配置：" + message });
    } finally {
      setBusy(false);
    }
  };

  const commit = async (conflictConfirmation?: string) => {
    if (!plan) return;
    setBusy(true);
    setError(null);
    try {
      const result = await commitOperation({
        domain: "asset",
        request: {
          operation_id: plan.operation_id,
          candidate_hash: plan.candidate_hash,
          conflict_confirmation: conflictConfirmation ?? null,
        },
      });
      if (result.domain !== "asset") {
        throw new Error("Core returned a Skill inventory for an Agent configuration commit");
      }
      await onSaved();
      toast.show({ kind: "success", msg: `${agent.name} 配置已更新。` });
      onClose();
    } catch (commitError) {
      setError(formatError(commitError));
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
        busy={busy}
        error={error}
        agentName={agent.name}
        onCommit={commit}
        onCancel={cancelPlan}
      />
    );
  }

  return (
    <DialogShell
      kind="editor"
      size="md"
      title="编辑配置"
      subtitle={agent.name}
      busy={busy}
      onClose={onClose}
      footerStart={<span className="mux-agent-config-hint">保存前将显示影响范围</span>}
      footerEnd={(
        <>
          <button type="button" className="btn-ghost" disabled={busy} onClick={onClose}>取消</button>
          <button type="button" className="btn-primary" disabled={!canSubmit} onClick={() => void save()}>
            {busy ? "检查中…" : "继续"}
          </button>
        </>
      )}
    >
      <div className="mux-agent-config-form">
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
      </div>
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
