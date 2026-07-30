import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { addAgent } from "../lib/api";
import type { AgentDefinitionInput } from "../lib/types";
import { formatError } from "../lib/format";
import { AgentGlyph } from "./brandIcons";
import { DialogShell } from "./DialogShell";
import { CheckIcon, LayersIcon, PackageIcon, SparklesIcon } from "./icons";
import { useToast } from "./Toast";

const FORMATS = [
  { value: "json", label: "JSON" },
  { value: "toml", label: "TOML" },
  { value: "yaml", label: "YAML" },
] as const;

const CATEGORIES = [
  { value: "coding-agent", labelKey: "agents.categoryCoding" },
  { value: "cli", labelKey: "agents.categoryCli" },
  { value: "ide", labelKey: "agents.categoryIde" },
  { value: "desktop", labelKey: "agents.categoryDesktop" },
] as const;

const AGENT_ID_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

function isSkillsDirectory(value: string) {
  if (!value.startsWith("~/") || !value.endsWith("/skills")) return false;
  const parts = value.slice(2).split("/");
  return !parts.some((part) => !part || part === "." || part === "..")
    && parts[0] !== ".mux";
}

function isHttpUrl(value: string) {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}

/** Create an Agent identity first, then declare the resource writers MUX may
 * manage for it. Custom Model writers are intentionally not exposed yet. */
export function AddAgentDialog({
  onClose,
  onAdded,
}: {
  onClose: () => void;
  onAdded: () => Promise<unknown> | void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [id, setId] = useState("");
  const [category, setCategory] = useState("coding-agent");
  const [mcpEnabled, setMcpEnabled] = useState(true);
  const [skillsEnabled, setSkillsEnabled] = useState(false);
  const [format, setFormat] = useState<"json" | "toml" | "yaml">("json");
  const [key, setKey] = useState("mcpServers");
  const [global, setGlobal] = useState("");
  const [skillsDir, setSkillsDir] = useState("");
  const [skillsDocs, setSkillsDocs] = useState("");
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const trimmedName = name.trim();
  const trimmedId = id.trim();
  const trimmedGlobal = global.trim();
  const trimmedKey = key.trim();
  const trimmedSkillsDir = skillsDir.trim();
  const trimmedSkillsDocs = skillsDocs.trim();
  const skillsTargetId = `${trimmedId}-skills`;
  const idValid = AGENT_ID_PATTERN.test(trimmedId)
    && trimmedId.length <= (skillsEnabled ? 57 : 64);
  const skillsDirValid = isSkillsDirectory(trimmedSkillsDir);
  const skillsDocsValid = isHttpUrl(trimmedSkillsDocs);
  const mcpValid = !mcpEnabled || (trimmedGlobal.length > 0 && trimmedKey.length > 0);
  const skillsValid = !skillsEnabled || (skillsDirValid && skillsDocsValid);
  const hasCapability = mcpEnabled || skillsEnabled;
  const canSubmit = trimmedName.length > 0
    && idValid
    && hasCapability
    && mcpValid
    && skillsValid
    && !busy;
  const selectedCapabilities = [
    mcpEnabled ? "MCP" : null,
    skillsEnabled ? "Skills" : null,
  ].filter(Boolean).join(" + ");
  const displayName = trimmedName || trimmedId || t("agents.newAgent");

  const submit = async () => {
    if (!canSubmit) return;
    setBusy(true);
    const definition: AgentDefinitionInput = {
      global: mcpEnabled ? trimmedGlobal : null,
      project: null,
      format: mcpEnabled ? format : "",
      key: mcpEnabled ? trimmedKey : "",
      enabled: true,
      builtin: false,
      name: trimmedName,
      category,
      evidence: "custom",
      verified_at: null,
      docs: skillsEnabled ? trimmedSkillsDocs : null,
      skills: skillsEnabled ? {
        target_id: skillsTargetId,
        global_dir: trimmedSkillsDir,
        aliases: [],
        docs: trimmedSkillsDocs,
        evidence: "official",
        verified_at: new Date().toISOString().slice(0, 10),
        probes: [{ kind: "path", path: trimmedSkillsDir }],
      } : null,
    };
    try {
      await addAgent(trimmedId, definition);
      toast.show({
        kind: "success",
        msg: t("agents.addedToast", { name: trimmedName }),
      });
      await onAdded();
      onClose();
    } catch (error) {
      toast.show({
        kind: "error",
        msg: t("agents.addFailed", { error: formatError(error) }),
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <DialogShell
      kind="editor"
      size="lg"
      title={t("agents.addTitle")}
      subtitle={t("agents.addSubtitle")}
      busy={busy}
      onClose={onClose}
      footerStart={(
        <span className="mux-agent-create-summary" data-empty={!hasCapability || undefined}>
          <span />
          {hasCapability
            ? t("agents.capabilitySummary", { capabilities: selectedCapabilities })
            : t("agents.capabilityRequired")}
        </span>
      )}
      footerEnd={(
        <>
          <button type="button" onClick={onClose} disabled={busy} className="btn-ghost">
            {t("common.cancel")}
          </button>
          <button
            type="button"
            disabled={!canSubmit}
            onClick={() => void submit()}
            className="btn-primary"
          >
            {busy ? t("agents.adding") : t("agents.addAction")}
          </button>
        </>
      )}
    >
      <div className="mux-agent-create">
        <section className="mux-agent-create-section" aria-labelledby="agent-identity-title">
          <div className="mux-agent-create-section-head">
            <div>
              <h3 id="agent-identity-title">{t("agents.identityTitle")}</h3>
              <p>{t("agents.identityHelp")}</p>
            </div>
          </div>

          <div className="mux-agent-identity-card">
            <div className="mux-agent-identity-preview" aria-label={t("agents.identityPreview")}>
              <AgentGlyph id={trimmedId || "new-agent"} name={displayName} size={46} />
              <span>
                <strong>{displayName}</strong>
                <code>{trimmedId || "agent-id"}</code>
              </span>
            </div>

            <div className="mux-agent-identity-fields">
              <label className="mux-agent-create-field">
                <span>{t("agents.nameLabel")} <i>*</i></span>
                <input
                  autoFocus
                  className="mux-model-field"
                  value={name}
                  placeholder={t("agents.namePlaceholder")}
                  onChange={(event) => setName(event.target.value)}
                />
              </label>
              <label className="mux-agent-create-field">
                <span>{t("agents.idLabel")} <i>*</i></span>
                <input
                  className="mux-model-field"
                  value={id}
                  spellCheck={false}
                  aria-invalid={id.length > 0 && !idValid}
                  placeholder={t("agents.idPlaceholder")}
                  onChange={(event) => setId(event.target.value)}
                />
                <small data-error={id.length > 0 && !idValid || undefined}>
                  {id.length > 0 && !idValid
                    ? t(skillsEnabled ? "agents.idInvalidWithSkills" : "agents.idInvalid")
                    : t("agents.idHelp")}
                </small>
              </label>
              <label className="mux-agent-create-field" data-wide>
                <span>{t("agents.categoryLabel")}</span>
                <select
                  className="mux-model-field"
                  value={category}
                  onChange={(event) => setCategory(event.target.value)}
                >
                  {CATEGORIES.map((option) => (
                    <option key={option.value} value={option.value}>
                      {t(option.labelKey)}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          </div>
        </section>

        <section className="mux-agent-create-section" aria-labelledby="agent-capabilities-title">
          <div className="mux-agent-create-section-head">
            <div>
              <h3 id="agent-capabilities-title">{t("agents.capabilitiesTitle")}</h3>
              <p>{t("agents.capabilitiesHelp")}</p>
            </div>
          </div>

          <div className="mux-agent-capability-grid">
            <CapabilityButton
              title="MCP"
              description={t("agents.mcpCapability")}
              icon={<PackageIcon className="w-4 h-4" />}
              selected={mcpEnabled}
              onClick={() => setMcpEnabled((current) => !current)}
              label={t("agents.toggleMcp")}
            />
            <CapabilityButton
              title="Skills"
              description={t("agents.skillsCapability")}
              icon={<SparklesIcon className="w-4 h-4" />}
              selected={skillsEnabled}
              onClick={() => setSkillsEnabled((current) => !current)}
              label={t("agents.toggleSkills")}
            />
            <button
              type="button"
              className="mux-agent-capability"
              data-unavailable
              disabled
              aria-label={t("agents.modelsUnavailable")}
            >
              <span className="mux-agent-capability-icon">
                <LayersIcon className="w-4 h-4" />
              </span>
              <span className="mux-agent-capability-copy">
                <strong>Models</strong>
                <small>{t("agents.modelsCapability")}</small>
              </span>
              <span className="mux-agent-capability-state">{t("agents.unavailable")}</span>
            </button>
          </div>

          {mcpEnabled && (
            <section className="mux-agent-capability-detail" aria-labelledby="agent-mcp-config-title">
              <div className="mux-agent-capability-detail-head">
                <span><PackageIcon className="w-4 h-4" /></span>
                <div>
                  <h4 id="agent-mcp-config-title">{t("agents.mcpConfigTitle")}</h4>
                  <p>{t("agents.mcpConfigHelp")}</p>
                </div>
              </div>
              <div className="mux-agent-detail-fields">
                <label className="mux-agent-create-field" data-wide>
                  <span>{t("agents.mcpPathLabel")} <i>*</i></span>
                  <input
                    className="mux-model-field"
                    value={global}
                    spellCheck={false}
                    placeholder={trimmedId
                      ? `~/.${trimmedId}/mcp.json`
                      : t("agents.mcpPathPlaceholder")}
                    onChange={(event) => setGlobal(event.target.value)}
                  />
                </label>
                <div className="mux-agent-create-field">
                  <span>{t("agents.formatLabel")}</span>
                  <div className="mux-agent-format-picker" role="group" aria-label={t("agents.formatLabel")}>
                    {FORMATS.map((candidate) => (
                      <button
                        type="button"
                        key={candidate.value}
                        aria-pressed={format === candidate.value}
                        onClick={() => setFormat(candidate.value)}
                      >
                        {candidate.label}
                      </button>
                    ))}
                  </div>
                </div>
                <label className="mux-agent-create-field">
                  <span>{t("agents.mcpKeyLabel")} <i>*</i></span>
                  <input
                    className="mux-model-field"
                    value={key}
                    spellCheck={false}
                    placeholder="mcpServers"
                    onChange={(event) => setKey(event.target.value)}
                  />
                  <small>{t("agents.mcpKeyHelp")}</small>
                </label>
              </div>
            </section>
          )}

          {skillsEnabled && (
            <section className="mux-agent-capability-detail" aria-labelledby="agent-skills-config-title">
              <div className="mux-agent-capability-detail-head">
                <span><SparklesIcon className="w-4 h-4" /></span>
                <div>
                  <h4 id="agent-skills-config-title">{t("agents.skillsConfigTitle")}</h4>
                  <p>{t("agents.skillsConfigHelp")}</p>
                </div>
              </div>
              <div className="mux-agent-detail-fields">
                <label className="mux-agent-create-field">
                  <span>{t("agents.skillsPathLabel")} <i>*</i></span>
                  <input
                    className="mux-model-field"
                    value={skillsDir}
                    spellCheck={false}
                    aria-invalid={skillsDir.length > 0 && !skillsDirValid}
                    placeholder={trimmedId
                      ? `~/.${trimmedId}/skills`
                      : t("agents.skillsPathPlaceholder")}
                    onChange={(event) => setSkillsDir(event.target.value)}
                  />
                  <small data-error={skillsDir.length > 0 && !skillsDirValid || undefined}>
                    {skillsDir.length > 0 && !skillsDirValid
                      ? t("agents.skillsPathInvalid")
                      : t("agents.skillsPathHelp")}
                  </small>
                </label>
                <label className="mux-agent-create-field">
                  <span>{t("agents.skillsDocsLabel")} <i>*</i></span>
                  <input
                    className="mux-model-field"
                    value={skillsDocs}
                    type="url"
                    spellCheck={false}
                    aria-invalid={skillsDocs.length > 0 && !skillsDocsValid}
                    placeholder="https://docs.example.com/skills"
                    onChange={(event) => setSkillsDocs(event.target.value)}
                  />
                  <small data-error={skillsDocs.length > 0 && !skillsDocsValid || undefined}>
                    {skillsDocs.length > 0 && !skillsDocsValid
                      ? t("agents.skillsDocsInvalid")
                      : t("agents.skillsDocsHelp")}
                  </small>
                </label>
              </div>
            </section>
          )}
        </section>
      </div>
    </DialogShell>
  );
}

function CapabilityButton({
  title,
  description,
  icon,
  selected,
  onClick,
  label,
}: {
  title: string;
  description: string;
  icon: ReactNode;
  selected: boolean;
  onClick: () => void;
  label: string;
}) {
  return (
    <button
      type="button"
      className="mux-agent-capability"
      role="switch"
      aria-checked={selected}
      aria-label={label}
      data-selected={selected || undefined}
      onClick={onClick}
    >
      <span className="mux-agent-capability-icon">{icon}</span>
      <span className="mux-agent-capability-copy">
        <strong>{title}</strong>
        <small>{description}</small>
      </span>
      <span className="mux-agent-capability-check" aria-hidden="true">
        {selected && <CheckIcon className="w-3 h-3" />}
      </span>
    </button>
  );
}
