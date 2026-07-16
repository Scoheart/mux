import { useCallback, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  applyModelProfile,
  deleteModelProfile,
  listModelAgents,
  listModelProfiles,
  saveModelProfile,
} from "../lib/api";
import type {
  ModelAgentView,
  ModelProfile,
  ModelProfileView,
  ModelProtocol,
} from "../lib/types";
import { formatError } from "../lib/format";
import { AgentGlyph } from "./brandIcons";
import { Badge, IconButton, Modal, ModalHeader } from "./ui";
import { CheckIcon, EditIcon, LinkIcon, PlusIcon, TrashIcon } from "./icons";
import { useToast } from "./Toast";
import { FeatureShell } from "./FeatureShell";
import { SelectMenu } from "./SelectMenu";

const PROTOCOLS: Array<{ id: ModelProtocol; label: string }> = [
  { id: "anthropic-messages", label: "Anthropic Messages" },
  { id: "openai-responses", label: "OpenAI Responses" },
  { id: "openai-completions", label: "OpenAI Completions" },
];

const emptyProfile = (): ModelProfile => ({
  id: "",
  name: "",
  protocol: "openai-responses",
  base_url: "",
  model: "",
  reasoning: false,
});

function protocolLabel(protocol: ModelProtocol) {
  return PROTOCOLS.find((item) => item.id === protocol)?.label ?? protocol;
}

export function ModelsView({ onSelectMcps }: { onSelectMcps: () => void }) {
  const [profiles, setProfiles] = useState<ModelProfileView[]>([]);
  const [agents, setAgents] = useState<ModelAgentView[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [selectedByAgent, setSelectedByAgent] = useState<Record<string, string>>({});
  const [editing, setEditing] = useState<ModelProfileView | null | undefined>(undefined);
  const [busyAgent, setBusyAgent] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const toast = useToast();

  const refresh = useCallback(async () => {
    const [nextProfiles, nextAgents] = await Promise.all([
      listModelProfiles(),
      listModelAgents(),
    ]);
    setProfiles(nextProfiles);
    setAgents(nextAgents);
    setSelectedByAgent(
      Object.fromEntries(
        nextAgents.map((agent) => [agent.id, agent.assigned_profile ?? ""])
      )
    );
    setSelectedProfileId((current) => {
      if (current && nextProfiles.some((profile) => profile.id === current)) return current;
      return nextProfiles[0]?.id ?? null;
    });
  }, []);

  useEffect(() => {
    refresh()
      .catch((error) =>
        toast.show({ kind: "error", msg: "读取模型配置失败：" + formatError(error) })
      )
      .finally(() => setLoading(false));
  }, [refresh]);

  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId) ?? null;

  const removeProfile = async (profile: ModelProfileView) => {
    if (!window.confirm(`删除模型接口「${profile.name}」？Agent 配置文件不会被回滚。`)) return;
    try {
      await deleteModelProfile(profile.id);
      await refresh();
      toast.show({ kind: "success", msg: `已删除：${profile.name}` });
    } catch (error) {
      toast.show({ kind: "error", msg: "删除失败：" + formatError(error) });
    }
  };

  const apply = async (agent: ModelAgentView) => {
    const profileId = selectedByAgent[agent.id];
    const profile = profiles.find((item) => item.id === profileId);
    if (!profile) return;
    if (
      !window.confirm(
        `将「${profile.name}」应用到 ${agent.name}？\n\n目标：${agent.config_path}\nMUX 会先创建备份。`
      )
    )
      return;
    setBusyAgent(agent.id);
    try {
      const result = await applyModelProfile(agent.id, profile.id);
      await refresh();
      toast.show({ kind: "success", msg: result.message });
    } catch (error) {
      toast.show({ kind: "error", msg: "应用失败：" + formatError(error) });
    } finally {
      setBusyAgent(null);
    }
  };

  if (loading) {
    return (
      <FeatureShell
        active="models"
        onSelectMcps={onSelectMcps}
        onSelectModels={() => {}}
        sidebar={
          <aside className="mux-feature-sidebar">
            <div className="flex items-center gap-1.5 px-3 pt-3.5 pb-2">
              <span
                className="text-xs font-semibold uppercase flex-1"
                style={{ color: "var(--text-secondary)", letterSpacing: "0.06em" }}
              >
                模型
              </span>
            </div>
          </aside>
        }
      >
        <div className="py-16 text-sm text-center" style={{ color: "var(--text-secondary)" }}>
          加载中…
        </div>
      </FeatureShell>
    );
  }

  return (
    <FeatureShell
      active="models"
      onSelectMcps={onSelectMcps}
      onSelectModels={() => {}}
      sidebar={
        <aside className="mux-feature-sidebar">
          <div className="flex items-center gap-1.5 px-3 pt-3.5 pb-2">
            <span
              className="text-xs font-semibold uppercase flex-1"
              style={{ color: "var(--text-secondary)", letterSpacing: "0.06em" }}
            >
              模型
            </span>
            <IconButton title="新建模型接口" onClick={() => setEditing(null)}>
              <PlusIcon className="w-4 h-4" />
            </IconButton>
          </div>

          <div className="flex-1 min-h-0 overflow-y-auto px-2 pb-3 mux-noscroll">
            {profiles.length === 0 ? (
              <button className="mux-model-empty" type="button" onClick={() => setEditing(null)}>
                <PlusIcon className="w-4 h-4" />
                新建模型接口
              </button>
            ) : (
              profiles.map((profile) => (
                <button
                  type="button"
                  key={profile.id}
                  className="mux-model-profile-row"
                  data-active={profile.id === selectedProfileId ? "true" : undefined}
                  onClick={() => setSelectedProfileId(profile.id)}
                >
                  <span className="mux-model-profile-dot" data-protocol={profile.protocol} />
                  <span className="min-w-0 flex-1">
                    <strong>{profile.name}</strong>
                    <small>{profile.model}</small>
                  </span>
                  {profile.credential_saved && (
                    <CheckIcon className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "#34C759" }} />
                  )}
                </button>
              ))
            )}
          </div>

          {selectedProfile && (
            <div className="mux-model-profile-actions">
              <button type="button" onClick={() => setEditing(selectedProfile)}>
                <EditIcon className="w-3.5 h-3.5" /> 编辑
              </button>
              <button type="button" data-danger="true" onClick={() => void removeProfile(selectedProfile)}>
                <TrashIcon className="w-3.5 h-3.5" /> 删除
              </button>
            </div>
          )}
        </aside>
      }
      toolbar={
        <div className="mux-feature-chrome-toolbar mux-models-toolbar">
          <div className="min-w-0 flex-1">
            <h1 className="mux-models-title">Agent Models</h1>
            <p className="mux-models-sub">首批支持 Claude Code、Codex、Pi 与 Qoder。</p>
          </div>
          {selectedProfile && (
            <div className="mux-model-selected-summary">
              <span>{protocolLabel(selectedProfile.protocol)}</span>
              <strong>{selectedProfile.name}</strong>
            </div>
          )}
        </div>
      }
    >
      <div className="mux-model-agent-list">
        {agents.map((agent) => {
          const compatible = profiles.filter((profile) =>
            agent.supported_protocols.includes(profile.protocol)
          );
          const assigned = agent.assigned_profile
            ? profiles.find((profile) => profile.id === agent.assigned_profile)
            : null;
          const selected = selectedByAgent[agent.id] ?? "";
          return (
            <article className="mux-model-agent-row" key={agent.id}>
              <AgentGlyph id={agent.id} name={agent.name} size={38} />
              <div className="mux-model-agent-copy">
                <div className="mux-model-agent-title">
                  <strong>{agent.name}</strong>
                  <Badge tone={agent.installed ? "success" : "neutral"}>
                    {agent.installed ? "已安装" : "未检测到"}
                  </Badge>
                  {agent.mode === "guided" && <Badge tone="warning">手动配置</Badge>}
                </div>
                <code>{agent.config_path}</code>
                {assigned && <small>当前：{assigned.name}</small>}
              </div>

              {agent.mode === "guided" ? (
                <button type="button" className="btn-ghost" onClick={() => openUrl(agent.docs)}>
                  <LinkIcon className="w-4 h-4" />
                  打开设置说明
                </button>
              ) : (
                <div className="mux-model-agent-controls">
                  <SelectMenu
                    stretch
                    aria-label={`${agent.name} 模型接口`}
                    value={selected}
                    placeholder="选择模型接口"
                    options={compatible.map((profile) => ({
                      value: profile.id,
                      label: profile.name,
                      meta: profile.model,
                    }))}
                    onChange={(next) =>
                      setSelectedByAgent((current) => ({
                        ...current,
                        [agent.id]: next,
                      }))
                    }
                  />
                  <button
                    type="button"
                    className="btn-primary"
                    disabled={!selected || busyAgent !== null}
                    onClick={() => void apply(agent)}
                  >
                    {busyAgent === agent.id ? "应用中…" : "应用"}
                  </button>
                </div>
              )}
            </article>
          );
        })}
      </div>

      {editing !== undefined && (
        <ModelProfileDialog
          initial={editing}
          onClose={() => setEditing(undefined)}
          onSaved={async () => {
            setEditing(undefined);
            await refresh();
          }}
        />
      )}
    </FeatureShell>
  );
}

function ModelProfileDialog({
  initial,
  onClose,
  onSaved,
}: {
  initial: ModelProfileView | null;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const [draft, setDraft] = useState<ModelProfile>(initial ?? emptyProfile());
  const [credential, setCredential] = useState("");
  const [clearCredential, setClearCredential] = useState(false);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const valid =
    draft.id.trim() &&
    draft.name.trim() &&
    draft.base_url.trim() &&
    draft.model.trim() &&
    !busy;

  const save = async () => {
    if (!valid) return;
    setBusy(true);
    try {
      const credentialUpdate = clearCredential
        ? ""
        : credential
          ? credential
          : undefined;
      await saveModelProfile(
        {
          ...draft,
          id: draft.id.trim(),
          name: draft.name.trim(),
          base_url: draft.base_url.trim().replace(/\/$/, ""),
          model: draft.model.trim(),
        },
        credentialUpdate
      );
      await onSaved();
      toast.show({ kind: "success", msg: initial ? "模型接口已更新" : "模型接口已创建" });
    } catch (error) {
      toast.show({ kind: "error", msg: "保存失败：" + formatError(error) });
    } finally {
      setBusy(false);
    }
  };

  const fieldClass = "mux-model-field";
  return (
    <Modal width={590} maxHeight="88vh" onClose={onClose}>
      <ModalHeader
        glyph="M"
        title={initial ? "编辑模型接口" : "新建模型接口"}
        subtitle="API Key 仅保存到 macOS Keychain。"
        onClose={onClose}
      />
      <div className="mux-model-form">
        <div className="mux-model-form-grid">
          <label>
            <span>名称</span>
            <input
              autoFocus
              className={fieldClass}
              value={draft.name}
              onChange={(event) => setDraft({ ...draft, name: event.target.value })}
              placeholder="公司网关"
            />
          </label>
          <label>
            <span>ID</span>
            <input
              className={fieldClass}
              value={draft.id}
              disabled={Boolean(initial)}
              onChange={(event) => setDraft({ ...draft, id: event.target.value.toLowerCase() })}
              placeholder="company-gateway"
              spellCheck={false}
            />
          </label>
        </div>

        <label>
          <span>协议</span>
          <SelectMenu
            stretch
            aria-label="协议"
            value={draft.protocol}
            options={PROTOCOLS.map((protocol) => ({
              value: protocol.id,
              label: protocol.label,
            }))}
            onChange={(next) =>
              setDraft({ ...draft, protocol: next as ModelProtocol })
            }
          />
        </label>

        <label>
          <span>Base URL</span>
          <input
            className={fieldClass}
            value={draft.base_url}
            onChange={(event) => setDraft({ ...draft, base_url: event.target.value })}
            placeholder="https://api.example.com/v1"
            spellCheck={false}
          />
        </label>

        <label>
          <span>模型 ID</span>
          <input
            className={fieldClass}
            value={draft.model}
            onChange={(event) => setDraft({ ...draft, model: event.target.value })}
            placeholder="model-name"
            spellCheck={false}
          />
        </label>

        <label>
          <span>API Key</span>
          <input
            type="password"
            autoComplete="new-password"
            className={fieldClass}
            value={credential}
            disabled={clearCredential}
            onChange={(event) => setCredential(event.target.value)}
            placeholder={initial?.credential_saved ? "留空保留现有密钥" : "本地无鉴权接口可留空"}
          />
        </label>

        {initial?.credential_saved && (
          <label className="mux-model-clear-key">
            <input
              type="checkbox"
              checked={clearCredential}
              onChange={(event) => setClearCredential(event.target.checked)}
            />
            清除已存密钥
          </label>
        )}

        <div className="mux-model-form-grid">
          <label>
            <span>上下文窗口（Pi）</span>
            <input
              type="number"
              min={1}
              className={fieldClass}
              value={draft.context_window ?? ""}
              onChange={(event) =>
                setDraft({
                  ...draft,
                  context_window: event.target.value ? Number(event.target.value) : undefined,
                })
              }
              placeholder="128000"
            />
          </label>
          <label>
            <span>最大输出（Pi）</span>
            <input
              type="number"
              min={1}
              className={fieldClass}
              value={draft.max_output_tokens ?? ""}
              onChange={(event) =>
                setDraft({
                  ...draft,
                  max_output_tokens: event.target.value ? Number(event.target.value) : undefined,
                })
              }
              placeholder="16384"
            />
          </label>
        </div>

        <label className="mux-model-clear-key">
          <input
            type="checkbox"
            checked={draft.reasoning}
            onChange={(event) => setDraft({ ...draft, reasoning: event.target.checked })}
          />
          推理模型（Pi）
        </label>
      </div>
      <footer className="mux-model-form-footer">
        <button type="button" className="btn-ghost" onClick={onClose}>取消</button>
        <button type="button" className="btn-primary" disabled={!valid} onClick={() => void save()}>
          {busy ? "保存中…" : "保存"}
        </button>
      </footer>
    </Modal>
  );
}
