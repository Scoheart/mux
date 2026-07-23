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
import { FolderIcon, LinkIcon, PackageIcon } from "./icons";
import { useToast } from "./Toast";
import { DialogShell } from "./DialogShell";

export interface SkillInstallDialogProps {
  plan: SkillsState["plan"];
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
  if (resolution.source.kind === "archive") {
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
  plan,
  commit,
  cancel,
  onClose,
  onCommitted,
  onRecoveryRequired,
}: SkillInstallDialogProps) {
  const toast = useToast();
  const [githubValue, setGithubValue] = useState("");
  const [wizard, dispatch] = useReducer(
    installWizardReducer,
    undefined,
    () => installWizardReducer(undefined, { type: "reset" }),
  );
  const [resolving, setResolving] = useState(false);
  const [planning, setPlanning] = useState(false);
  const [committing, setCommitting] = useState(false);
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

  const resolveArchive = () => {
    if (resolving) return;
    void loadResolution(api.resolveArchiveSkillSourceDialog());
  };

  const returnToSource = async () => {
    if (planning || committing || closing) return;
    const resolution = resolutionRef.current;
    planGenerationRef.current += 1;
    if (resolution) await cancelOnce(resolution.operation_id, true);
    resolutionRef.current = null;
    dispatch({ type: "reset" });
    setPlanError(null);
  };

  const commitInstall: SkillsState["commit"] = (plan, confirmation) => {
    setCommitting(true);
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
        if (mountedRef.current) setCommitting(false);
      })
      .catch(() => undefined);
    return pending;
  };

  const finishInstall = (inventory: SkillsInventory) => {
    committedRef.current = true;
    onCommitted(inventory);
    onClose();
  };

  const addSelected = async () => {
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
      const nextPlan = await plan({
        operation: "install_skill",
        request: {
          resolution_id: resolution.operation_id,
          skill_names: wizard.selectedSkillNames,
          replace_conflicts: wizard.replaceConflicts,
        },
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
      if (nextPlan.operation_id !== resolution.operation_id) {
        await cancelOnce(nextPlan.operation_id, false);
        setPlanError({
          code: "protocol_error",
          message: "安装计划未绑定当前来源，请重新读取来源。",
        });
        return;
      }
      try {
        const inventory = await commitInstall(
          nextPlan,
          nextPlan.requires_risk_override ? nextPlan.findings_hash : null,
        );
        if (mountedRef.current && !closedRef.current) finishInstall(inventory);
      } catch (reason) {
        if (!mountedRef.current || closedRef.current) return;
        const error = normalizeSkillCommandError(reason);
        if (error.code === "recovery_required") {
          enterRecovery(error.message);
        } else {
          setPlanError(error);
        }
      }
    } catch (reason) {
      if (
        mountedRef.current &&
        !closedRef.current &&
        planGenerationRef.current === generation
      ) {
        const error = normalizeSkillCommandError(reason);
        setPlanError(error);
        if (error.code === "conflict") {
          dispatch({ type: "set_replace_conflicts", enabled: true });
        }
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

  function enterRecovery(message: string) {
    recoveryRequiredRef.current = true;
    onRecoveryRequired(message);
    onClose();
  }

  const resolution = wizard.resolution;
  const selectedCount = wizard.selectedSkillNames.length;
  const candidateCount = resolution?.candidates.length ?? 0;
  const busy = resolving || planning || committing || closing;
  const actionVerb = resolution?.source.kind === "github" ? "下载" : "导入";
  const addLabel = wizard.replaceConflicts
    ? `备份并${actionVerb}`
    : selectedCount > 1
      ? `${actionVerb} ${selectedCount} 个 Skill`
      : `${actionVerb} Skill`;

  return (
    <DialogShell
      kind="editor"
      size="md"
      title="添加 Skill"
      subtitle="从 GitHub 下载，或从本地直接导入。"
      busy={busy}
      closeLabel="关闭"
      onClose={() => void closeDialog()}
      footerStart={resolution ? <span className="mux-skill-selection-count">已选 {selectedCount} / {candidateCount}</span> : undefined}
      footerEnd={resolution ? (
        <>
          <button type="button" className="btn-ghost" disabled={busy} onClick={() => void returnToSource()}>
            更换来源
          </button>
          <button
            type="button"
            className="btn-primary"
            disabled={selectedCount === 0 || busy}
            onClick={() => void addSelected()}
          >
            {planning || committing ? `${actionVerb}中…` : addLabel}
          </button>
        </>
      ) : undefined}
    >
      <div className="mux-skill-install-dialog">
        <div className="mux-skill-dialog-body">
          {!resolution ? (
            <div className="mux-skill-source-step mux-skill-source-step-compact">
              <section>
                <div className="mux-skill-source-heading">
                  <LinkIcon className="w-4 h-4" />
                  <div>
                    <h3>GitHub</h3>
                  </div>
                </div>
                <label htmlFor="mux-skill-github-source">仓库地址</label>
                <div className="mux-skill-source-input-row">
                  <input
                    id="mux-skill-github-source"
                    data-modal-initial-focus
                    type="text"
                    value={githubValue}
                    disabled={resolving || closing}
                    placeholder="owner/repo 或 GitHub URL"
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
                    {resolving ? "查找中…" : "查找"}
                  </button>
                </div>
              </section>

              <button
                type="button"
                className="mux-skill-local-source"
                aria-label="选择本地文件夹"
                disabled={resolving || closing}
                onClick={resolveLocal}
              >
                <span className="mux-skill-local-source-icon"><FolderIcon className="w-4 h-4" /></span>
                <span>
                  <strong>文件夹</strong>
                  <small>本机目录</small>
                </span>
              </button>
              <button
                type="button"
                className="mux-skill-local-source"
                aria-label="选择 Skill 压缩包"
                disabled={resolving || closing}
                onClick={resolveArchive}
              >
                <span className="mux-skill-local-source-icon"><PackageIcon className="w-4 h-4" /></span>
                <span>
                  <strong>压缩包</strong>
                  <small>.zip · .tar.gz</small>
                </span>
              </button>

              {sourceError && (
                <div className="mux-skill-dialog-error" role="alert">
                  <strong>{sourceError.message}</strong>
                  {sourceError.retry_at && <code>可重试时间：{sourceError.retry_at}</code>}
                </div>
              )}
            </div>
          ) : (
            <div className="mux-skill-selection-step">
              <div className="mux-skill-resolution-summary">
                {resolution.source.kind === "github" ? (
                  <LinkIcon className="w-4 h-4" />
                ) : resolution.source.kind === "archive" ? (
                  <PackageIcon className="w-4 h-4" />
                ) : (
                  <FolderIcon className="w-4 h-4" />
                )}
                <span>
                  <strong>{sourceSummary(resolution)}</strong>
                  <code>
                    {resolution.resolved_revision ??
                      (resolution.source.kind === "archive" ? "本地压缩包" : "本地文件夹")}
                  </code>
                </span>
              </div>

              <section>
                <div className="mux-skill-selection-heading">
                  <h3>选择 Skill</h3>
                  <span>{candidateCount} 项</span>
                </div>
                <div className="mux-skill-choice-list">
                  {resolution.candidates.map((candidate) => (
                    <label key={candidate.name}>
                      <input
                        type="checkbox"
                        aria-label={candidate.name}
                        checked={wizard.selectedSkillNames.includes(candidate.name)}
                        disabled={planning || closing}
                        onChange={() => {
                          setPlanError(null);
                          dispatch({ type: "set_replace_conflicts", enabled: false });
                          dispatch({ type: "toggle_skill", skillName: candidate.name });
                        }}
                      />
                      <span>
                        <strong>{candidate.name}</strong>
                        <small>{candidate.description}</small>
                      </span>
                    </label>
                  ))}
                </div>
              </section>

              {planError && (
                <div
                  className={wizard.replaceConflicts ? "mux-skill-conflict-prompt" : "mux-skill-dialog-error"}
                  role="alert"
                >
                  <strong>{wizard.replaceConflicts ? "发现冲突" : planError.message}</strong>
                  {wizard.replaceConflicts && <span>{planError.message}</span>}
                  {wizard.replaceConflicts && <small>再次操作会先备份原内容。</small>}
                  {planError.retry_at && <code>可重试时间：{planError.retry_at}</code>}
                </div>
              )}
            </div>
          )}
        </div>

      </div>
    </DialogShell>
  );
}
