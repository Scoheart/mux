import {
  useCallback,
  useEffect,
  useReducer,
  useRef,
  useState,
} from "react";
import {
  normalizeSkillCommandError,
  type SkillsState,
} from "../hooks/useSkillsState";
import * as api from "../lib/api";
import { installWizardReducer } from "../lib/skills";
import type {
  SkillAgentView,
  SkillCommandError,
  SkillSourceResolution,
  SkillsInventory,
} from "../lib/types";
import { FolderIcon, LinkIcon, PackageIcon, XIcon } from "./icons";
import { SkillReviewDialog } from "./SkillReviewDialog";
import { useToast } from "./Toast";
import { Modal } from "./ui";

type InstallStep = "source" | "selection" | "review";

export interface SkillInstallDialogProps {
  agents: SkillAgentView[];
  commit: SkillsState["commit"];
  cancel: SkillsState["cancel"];
  onClose(): void;
  onCommitted(inventory: SkillsInventory): void;
  onRecoveryRequired(message: string): void;
  initialAgentId?: string;
}

function sourceSummary(resolution: SkillSourceResolution) {
  if (resolution.source.kind === "github") {
    const ref = resolution.source.requested_ref || "默认分支";
    const subpath = resolution.source.subpath ? ` / ${resolution.source.subpath}` : "";
    return `${resolution.source.owner}/${resolution.source.repo} · ${ref}${subpath}`;
  }
  if (resolution.source.kind === "local") {
    return `${resolution.source.path}${resolution.source.subpath ? ` / ${resolution.source.subpath}` : ""}`;
  }
  return resolution.source.original_path;
}

function selectedSnapshot(
  skillNames: string[],
  agentIds: string[],
  replaceConflicts: boolean,
) {
  return `${skillNames.join("\u0000")}\u0001${agentIds.join("\u0000")}\u0001${replaceConflicts}`;
}

function verifiedAgentSelection(
  selectedAgentIds: string[],
  agents: SkillAgentView[],
) {
  const verifiedAgentIds = new Set(agents.map((agent) => agent.id));
  return selectedAgentIds.filter((agentId) => verifiedAgentIds.has(agentId));
}

export function SkillInstallDialog({
  agents,
  commit,
  cancel,
  onClose,
  onCommitted,
  onRecoveryRequired,
  initialAgentId,
}: SkillInstallDialogProps) {
  const toast = useToast();
  const [step, setStep] = useState<InstallStep>("source");
  const [githubValue, setGithubValue] = useState("");
  const [wizard, dispatch] = useReducer(
    installWizardReducer,
    undefined,
    () => installWizardReducer(undefined, { type: "reset" }),
  );
  const [resolving, setResolving] = useState(false);
  const [planning, setPlanning] = useState(false);
  const [closing, setClosing] = useState(false);
  const [sourceError, setSourceError] = useState<SkillCommandError | null>(null);
  const [planError, setPlanError] = useState<SkillCommandError | null>(null);
  const mountedRef = useRef(true);
  const closedRef = useRef(false);
  const committedRef = useRef(false);
  const recoveryRequiredRef = useRef(false);
  const resolveGenerationRef = useRef(0);
  const planGenerationRef = useRef(0);
  const resolutionRef = useRef<SkillSourceResolution | null>(null);
  const commitPromiseRef = useRef<Promise<SkillsInventory> | null>(null);
  const closePromiseRef = useRef<Promise<void> | null>(null);
  const cancellationRefs = useRef(new Map<string, Promise<void>>());
  const cancelRef = useRef(cancel);
  const toastRef = useRef(toast);
  const agentsRef = useRef(agents);
  const initialAgentIdRef = useRef(initialAgentId);
  const wizardRef = useRef(wizard);
  cancelRef.current = cancel;
  toastRef.current = toast;
  agentsRef.current = agents;
  initialAgentIdRef.current = initialAgentId;
  wizardRef.current = wizard;
  const verifiedSelectedAgentIds = verifiedAgentSelection(
    wizard.selectedAgentIds,
    agents,
  );

  const cancelOnce = useCallback(
    (operationId: string, reportError: boolean) => {
      const existing = cancellationRefs.current.get(operationId);
      if (existing) return existing;
      const pending = cancelRef.current(operationId).catch((reason: unknown) => {
        if (reportError) {
          toastRef.current.show({
            kind: "error",
            msg: normalizeSkillCommandError(reason).message,
          });
        }
      });
      cancellationRefs.current.set(operationId, pending);
      return pending;
    },
    [],
  );

  const closeDialog = useCallback(() => {
    if (closePromiseRef.current) return closePromiseRef.current;
    closedRef.current = true;
    resolveGenerationRef.current += 1;
    planGenerationRef.current += 1;
    setClosing(true);

    const pending = (async () => {
      const committing = commitPromiseRef.current;
      if (committing) {
        try {
          await committing;
        } catch {
          // A failed commit leaves the shared resolution staged for cleanup.
        }
      }
      const resolution = resolutionRef.current;
      if (
        !committedRef.current &&
        !recoveryRequiredRef.current &&
        resolution
      ) {
        await cancelOnce(resolution.operation_id, true);
      }
      onClose();
    })();
    closePromiseRef.current = pending;
    return pending;
  }, [cancelOnce, onClose]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      closedRef.current = true;
      resolveGenerationRef.current += 1;
      planGenerationRef.current += 1;
      const resolution = resolutionRef.current;
      if (
        !committedRef.current &&
        !recoveryRequiredRef.current &&
        resolution
      ) {
        void (async () => {
          const committing = commitPromiseRef.current;
          if (committing) {
            try {
              await committing;
            } catch {
              // Cleanup below owns a failed commit's staging operation.
            }
          }
          if (!committedRef.current && !recoveryRequiredRef.current) {
            await cancelOnce(resolution.operation_id, false);
          }
        })();
      }
    };
  }, [cancelOnce]);

  useEffect(() => {
    const verifiedAgentIds = new Set(agents.map((agent) => agent.id));
    for (const agentId of wizard.selectedAgentIds) {
      if (!verifiedAgentIds.has(agentId)) {
        dispatch({ type: "toggle_agent", agentId });
      }
    }
  }, [agents, wizard.selectedAgentIds]);

  const loadResolution = async (
    pendingResolution: Promise<SkillSourceResolution | null>,
  ) => {
    const generation = ++resolveGenerationRef.current;
    setResolving(true);
    setSourceError(null);
    try {
      const resolution = await pendingResolution;
      if (!resolution) return;
      if (
        closedRef.current ||
        !mountedRef.current ||
        resolveGenerationRef.current !== generation
      ) {
        await cancelOnce(resolution.operation_id, false);
        return;
      }

      resolutionRef.current = resolution;
      dispatch({ type: "resolution_loaded", resolution });
      const currentInitialAgentId = initialAgentIdRef.current;
      if (
        currentInitialAgentId &&
        agentsRef.current.some((agent) => agent.id === currentInitialAgentId)
      ) {
        dispatch({ type: "toggle_agent", agentId: currentInitialAgentId });
      }
      setStep("selection");
    } catch (reason) {
      if (
        !closedRef.current &&
        mountedRef.current &&
        resolveGenerationRef.current === generation
      ) {
        setSourceError(normalizeSkillCommandError(reason));
      }
    } finally {
      if (
        !closedRef.current &&
        mountedRef.current &&
        resolveGenerationRef.current === generation
      ) {
        setResolving(false);
      }
    }
  };

  const resolveGithub = () => {
    const value = githubValue.trim();
    if (!value || resolving) return;
    void loadResolution(api.resolveGithubSkillSource(value));
  };

  const resolveLocal = () => {
    if (resolving) return;
    void loadResolution(api.resolveLocalSkillSourceDialog());
  };

  const returnToSource = async () => {
    if (planning || closing) return;
    const resolution = resolutionRef.current;
    planGenerationRef.current += 1;
    if (resolution) await cancelOnce(resolution.operation_id, true);
    resolutionRef.current = null;
    dispatch({ type: "reset" });
    setPlanError(null);
    setStep("source");
  };

  const reviewInstall = async () => {
    const resolution = resolutionRef.current;
    if (
      !resolution ||
      wizard.selectedSkillNames.length === 0 ||
      planning ||
      closing
    ) {
      return;
    }
    const generation = ++planGenerationRef.current;
    const selectedAgentIds = verifiedAgentSelection(
      wizard.selectedAgentIds,
      agentsRef.current,
    );
    const snapshot = selectedSnapshot(
      wizard.selectedSkillNames,
      selectedAgentIds,
      wizard.replaceConflicts,
    );
    setPlanning(true);
    setPlanError(null);
    try {
      const plan = await api.planSkillInstall({
        resolution_id: resolution.operation_id,
        skill_names: wizard.selectedSkillNames,
        agent_ids: selectedAgentIds,
        replace_conflicts: wizard.replaceConflicts,
      });
      const currentWizard = wizardRef.current;
      const stillCurrent =
        mountedRef.current &&
        !closedRef.current &&
        planGenerationRef.current === generation &&
        snapshot ===
          selectedSnapshot(
            currentWizard.selectedSkillNames,
            verifiedAgentSelection(
              currentWizard.selectedAgentIds,
              agentsRef.current,
            ),
            currentWizard.replaceConflicts,
          );
      if (!stillCurrent) {
        return;
      }
      if (plan.operation_id !== resolution.operation_id) {
        await cancelOnce(plan.operation_id, false);
        setPlanError({
          code: "protocol_error",
          message: "安装计划未绑定当前来源，请重新解析来源。",
        });
        return;
      }
      dispatch({ type: "plan_loaded", plan });
      setStep("review");
    } catch (reason) {
      if (
        mountedRef.current &&
        !closedRef.current &&
        planGenerationRef.current === generation
      ) {
        setPlanError(normalizeSkillCommandError(reason));
      }
    } finally {
      if (
        mountedRef.current &&
        !closedRef.current &&
        planGenerationRef.current === generation
      ) {
        setPlanning(false);
      }
    }
  };

  const commitInstall: SkillsState["commit"] = (plan, confirmation) => {
    const pending = commit(plan, confirmation);
    commitPromiseRef.current = pending;
    void pending
      .then(
        () => {
          committedRef.current = true;
        },
        (reason: unknown) => {
          if (normalizeSkillCommandError(reason).code === "recovery_required") {
            recoveryRequiredRef.current = true;
          }
          throw reason;
        },
      )
      .finally(() => {
        if (commitPromiseRef.current === pending) commitPromiseRef.current = null;
      })
      .catch(() => undefined);
    return pending;
  };

  const finishInstall = (inventory: SkillsInventory) => {
    committedRef.current = true;
    onCommitted(inventory);
    onClose();
  };

  const backFromReview = () => {
    if (commitPromiseRef.current) return;
    planGenerationRef.current += 1;
    setStep("selection");
  };

  const enterRecovery = (message: string) => {
    recoveryRequiredRef.current = true;
    onRecoveryRequired(message);
    onClose();
  };

  const agentNames = new Map(agents.map((agent) => [agent.id, agent.name]));
  const selectedAgents = new Set(verifiedSelectedAgentIds);
  const impactRows = wizard.plan?.targets.map((target) => {
    const alsoAffected = target.affected_agent_ids
      .filter((id) => !selectedAgents.has(id))
      .map((id) => agentNames.get(id) ?? id);
    return { target, alsoAffected };
  });

  return (
    <Modal
      ariaLabel="安装 Skill"
      width={760}
      maxHeight="min(88vh, 720px)"
      onClose={() => void closeDialog()}
    >
      <div className="mux-skill-install-dialog">
        <header className="mux-skill-dialog-header">
          <span className="mux-skill-dialog-glyph" aria-hidden="true">
            <PackageIcon className="w-5 h-5" />
          </span>
          <div>
            <h2 data-modal-title tabIndex={-1}>
              {step === "source"
                ? "安装 Skill"
                : step === "selection"
                  ? "选择 Skills 与 Agent"
                  : "审阅安装"}
            </h2>
            <p>来源只用于生成本地候选；写盘前仍需审阅完整计划。</p>
          </div>
          <button
            type="button"
            className="mux-skill-dialog-close"
            aria-label="关闭安装"
            title="关闭安装"
            disabled={closing}
            onClick={() => void closeDialog()}
          >
            <XIcon className="w-4 h-4" />
          </button>
        </header>

        <div className="mux-skill-dialog-steps" aria-label="安装步骤">
          {["来源", "选择", "审阅"].map((label, index) => (
            <span
              key={label}
              data-active={
                index === (step === "source" ? 0 : step === "selection" ? 1 : 2)
                  ? "true"
                  : undefined
              }
            >
              {index + 1}. {label}
            </span>
          ))}
        </div>

        <div className="mux-skill-dialog-body">
          {step === "source" ? (
            <div className="mux-skill-source-step">
              <section>
                <div className="mux-skill-source-heading">
                  <LinkIcon className="w-4 h-4" />
                  <div>
                    <h3>公开 GitHub 仓库</h3>
                    <p>支持 owner/repo 或 GitHub tree URL。</p>
                  </div>
                </div>
                <label htmlFor="mux-skill-github-source">GitHub 来源</label>
                <div className="mux-skill-source-input-row">
                  <input
                    id="mux-skill-github-source"
                    data-modal-initial-focus
                    type="text"
                    value={githubValue}
                    disabled={resolving || closing}
                    placeholder="owner/repo"
                    onChange={(event) => setGithubValue(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        event.preventDefault();
                        resolveGithub();
                      }
                    }}
                  />
                  <button
                    type="button"
                    className="btn-primary"
                    disabled={!githubValue.trim() || resolving || closing}
                    onClick={resolveGithub}
                  >
                    {resolving ? "解析中…" : "解析来源"}
                  </button>
                </div>
              </section>

              <div className="mux-skill-source-divider"><span>或</span></div>

              <section>
                <div className="mux-skill-source-heading">
                  <FolderIcon className="w-4 h-4" />
                  <div>
                    <h3>本地快照</h3>
                    <p>仅通过系统文件夹选择器读取，不接受手输路径。</p>
                  </div>
                </div>
                <button
                  type="button"
                  className="btn-secondary"
                  disabled={resolving || closing}
                  onClick={resolveLocal}
                >
                  选择本地文件夹
                </button>
              </section>

              {sourceError && (
                <div className="mux-skill-dialog-error" role="alert">
                  <strong>{sourceError.message}</strong>
                  {sourceError.retry_at && <code>可重试时间：{sourceError.retry_at}</code>}
                </div>
              )}
            </div>
          ) : (
            <div className="mux-skill-selection-step">
              {wizard.resolution && (
                <div className="mux-skill-resolution-summary">
                  <span>已解析来源</span>
                  <strong>{sourceSummary(wizard.resolution)}</strong>
                  <code>{wizard.resolution.resolved_revision ?? "本地快照"}</code>
                </div>
              )}

              <section>
                <div className="mux-skill-selection-heading">
                  <div>
                    <h3>Skills</h3>
                    <p>已默认选择本次发现的全部候选。</p>
                  </div>
                  <span>{wizard.selectedSkillNames.length} 项</span>
                </div>
                <div className="mux-skill-choice-list">
                  {wizard.resolution?.candidates.map((candidate) => (
                    <label key={candidate.name}>
                      <input
                        type="checkbox"
                        aria-label={candidate.name}
                        checked={wizard.selectedSkillNames.includes(candidate.name)}
                        disabled={planning || closing}
                        onChange={() =>
                          dispatch({ type: "toggle_skill", skillName: candidate.name })
                        }
                      />
                      <span>
                        <strong>{candidate.name}</strong>
                        <small>{candidate.description}</small>
                        <code>
                          {candidate.relative_path} · {candidate.file_count} 个文件 · {candidate.total_bytes} bytes
                        </code>
                      </span>
                    </label>
                  ))}
                </div>
              </section>

              <section>
                <div className="mux-skill-selection-heading">
                  <div>
                    <h3>目标 Agent</h3>
                    <p>默认不启用任何 Agent；共享目录会在计划中归一化。</p>
                  </div>
                  <span>{verifiedSelectedAgentIds.length} 个</span>
                </div>
                <div className="mux-skill-agent-choice-grid">
                  {agents.map((agent) => (
                    <label key={agent.id}>
                      <input
                        type="checkbox"
                        aria-label={agent.name}
                        checked={verifiedSelectedAgentIds.includes(agent.id)}
                        disabled={planning || closing}
                        onChange={() =>
                          dispatch({ type: "toggle_agent", agentId: agent.id })
                        }
                      />
                      <span>
                        <strong>{agent.name}</strong>
                        <code>{agent.global_dir}</code>
                      </span>
                    </label>
                  ))}
                </div>
              </section>

              <section>
                <div className="mux-skill-selection-heading">
                  <div>
                    <h3>冲突处理</h3>
                    <p>只允许替换同名中央副本；Agent 目录冲突仍会停止操作。</p>
                  </div>
                </div>
                <div className="mux-skill-choice-list">
                  <label>
                    <input
                      type="checkbox"
                      aria-label="备份并替换同名中央副本"
                      checked={wizard.replaceConflicts}
                      disabled={planning || closing}
                      onChange={(event) =>
                        dispatch({
                          type: "set_replace_conflicts",
                          enabled: event.target.checked,
                        })
                      }
                    />
                    <span>
                      <strong>备份并替换同名中央副本</strong>
                      <small>替换前会在 ~/.mux/backups/skills/ 保留原副本。</small>
                    </span>
                  </label>
                </div>
              </section>

              {step === "review" && impactRows && (
                <section className="mux-skill-install-impact" aria-label="共享目标影响">
                  {impactRows.map(({ target, alsoAffected }) => (
                    <div key={target.target_id}>
                      <code>{target.global_dir}</code>
                      {alsoAffected.length > 0 && (
                        <p>该共享目录也会被 {alsoAffected.join("、")} 读取。</p>
                      )}
                    </div>
                  ))}
                </section>
              )}

              {planError && (
                <div className="mux-skill-dialog-error" role="alert">
                  <strong>{planError.message}</strong>
                  {planError.retry_at && <code>可重试时间：{planError.retry_at}</code>}
                </div>
              )}
            </div>
          )}
        </div>

        {step !== "source" && (
          <footer className="mux-skill-dialog-footer">
            <button
              type="button"
              className="btn-secondary"
              disabled={planning || closing}
              onClick={() => void returnToSource()}
            >
              返回来源
            </button>
            <button
              type="button"
              className="btn-primary"
              disabled={
                wizard.selectedSkillNames.length === 0 || planning || closing
              }
              onClick={() => void reviewInstall()}
            >
              {planning ? "生成计划中…" : "审阅安装"}
            </button>
          </footer>
        )}
      </div>

      {step === "review" && wizard.plan && (
        <SkillReviewDialog
          plan={wizard.plan}
          onCommit={commitInstall}
          onClose={backFromReview}
          onCommitted={finishInstall}
          onRecoveryRequired={enterRecovery}
        />
      )}
    </Modal>
  );
}
