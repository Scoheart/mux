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
  SkillCommandError,
  SkillSourceResolution,
  SkillsInventory,
} from "../lib/types";
import { FolderIcon, LinkIcon } from "./icons";
import { SkillReviewDialog } from "./SkillReviewDialog";
import { useToast } from "./Toast";
import { DialogShell } from "./DialogShell";

type InstallStep = "source" | "selection" | "review";

export interface SkillInstallDialogProps {
  commit: SkillsState["commit"];
  cancel: SkillsState["cancel"];
  onClose(): void;
  onCommitted(inventory: SkillsInventory): void;
  onRecoveryRequired(message: string): void;
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
  replaceConflicts: boolean,
) {
  return `${skillNames.join("\u0000")}\u0001${replaceConflicts}`;
}

export function SkillInstallDialog({
  commit,
  cancel,
  onClose,
  onCommitted,
  onRecoveryRequired,
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
  const wizardRef = useRef(wizard);
  cancelRef.current = cancel;
  toastRef.current = toast;
  wizardRef.current = wizard;

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
    const snapshot = selectedSnapshot(
      wizard.selectedSkillNames,
      wizard.replaceConflicts,
    );
    setPlanning(true);
    setPlanError(null);
    try {
      const plan = await api.planSkillAssetInstall({
        resolution_id: resolution.operation_id,
        skill_names: wizard.selectedSkillNames,
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

  return (
    <DialogShell
      kind="editor"
      size="lg"
      title={step === "source" ? "添加 Skill 到资产库" : step === "selection" ? "选择中央 Skills" : "审阅中央入库"}
      subtitle="这里只维护中央资产；Agent 消费关系在各 Agent 或资产详情中单独管理。"
      busy={closing}
      closeLabel="关闭安装"
      onClose={() => void closeDialog()}
      footerEnd={step !== "source" ? (
        <>
          <button type="button" className="btn-secondary" disabled={planning || closing} onClick={() => void returnToSource()}>
            返回来源
          </button>
          <button
            type="button"
            className="btn-primary"
            disabled={wizard.selectedSkillNames.length === 0 || planning || closing}
            onClick={() => void reviewInstall()}
          >
            {planning ? "生成计划中…" : "审阅安装"}
          </button>
        </>
      ) : undefined}
    >
      <div className="mux-skill-install-dialog">
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

              {planError && (
                <div className="mux-skill-dialog-error" role="alert">
                  <strong>{planError.message}</strong>
                  {planError.retry_at && <code>可重试时间：{planError.retry_at}</code>}
                </div>
              )}
            </div>
          )}
        </div>

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
    </DialogShell>
  );
}
