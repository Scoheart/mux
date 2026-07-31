import {
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import { useTranslation } from "react-i18next";
import { addAgent } from "../lib/api";
import type { AgentDefinitionInput } from "../lib/types";
import { formatError } from "../lib/format";
import { AgentGlyph } from "./brandIcons";
import { DialogShell } from "./DialogShell";
import { PackageIcon, SparklesIcon } from "./icons";
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
type CapabilityTab = "mcp" | "skills";
type CapabilityState = "optional" | "incomplete" | "configured";

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
 * manage for it. A completed location includes that writer; an empty location
 * omits it. Custom Model writers require a dedicated safe adapter and are not
 * represented as a generic path-only capability. */
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
  const [activeCapability, setActiveCapability] = useState<CapabilityTab>("mcp");
  const [format, setFormat] = useState<"json" | "toml" | "yaml">("json");
  const [key, setKey] = useState("mcpServers");
  const [global, setGlobal] = useState("");
  const [skillsDir, setSkillsDir] = useState("");
  const [skillsDocs, setSkillsDocs] = useState("");
  const [busy, setBusy] = useState(false);
  const mcpTabRef = useRef<HTMLButtonElement>(null);
  const skillsTabRef = useRef<HTMLButtonElement>(null);
  const toast = useToast();

  const trimmedName = name.trim();
  const trimmedId = id.trim();
  const trimmedGlobal = global.trim();
  const trimmedKey = key.trim();
  const trimmedSkillsDir = skillsDir.trim();
  const trimmedSkillsDocs = skillsDocs.trim();
  const skillsTargetId = `${trimmedId}-skills`;
  const mcpStarted = trimmedGlobal.length > 0;
  const skillsStarted = trimmedSkillsDir.length > 0 || trimmedSkillsDocs.length > 0;
  const idValid = AGENT_ID_PATTERN.test(trimmedId)
    && trimmedId.length <= (skillsStarted ? 57 : 64);
  const skillsDirValid = isSkillsDirectory(trimmedSkillsDir);
  const skillsDocsValid = isHttpUrl(trimmedSkillsDocs);
  const mcpConfigured = mcpStarted && trimmedKey.length > 0;
  const skillsConfigured = skillsDirValid && skillsDocsValid;
  const mcpState: CapabilityState = mcpConfigured
    ? "configured"
    : mcpStarted ? "incomplete" : "optional";
  const skillsState: CapabilityState = skillsConfigured
    ? "configured"
    : skillsStarted ? "incomplete" : "optional";
  const incompleteCapabilities = [
    mcpState === "incomplete" ? "MCP" : null,
    skillsState === "incomplete" ? "Skills" : null,
  ].filter(Boolean).join(" + ");
  const hasCapability = mcpConfigured || skillsConfigured;
  const canSubmit = trimmedName.length > 0
    && idValid
    && hasCapability
    && !incompleteCapabilities
    && !busy;
  const selectedCapabilities = [
    mcpConfigured ? "MCP" : null,
    skillsConfigured ? "Skills" : null,
  ].filter(Boolean).join(" + ");
  const displayName = trimmedName || trimmedId || t("agents.newAgent");
  const handleCapabilityTabKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    let nextTab: CapabilityTab | null = null;
    if (event.key === "ArrowRight" || event.key === "End") nextTab = "skills";
    if (event.key === "ArrowLeft" || event.key === "Home") nextTab = "mcp";
    if (!nextTab) return;
    event.preventDefault();
    setActiveCapability(nextTab);
    (nextTab === "mcp" ? mcpTabRef : skillsTabRef).current?.focus();
  };

  const submit = async () => {
    if (!canSubmit) return;
    setBusy(true);
    const definition: AgentDefinitionInput = {
      global: mcpConfigured ? trimmedGlobal : null,
      project: null,
      format: mcpConfigured ? format : "",
      key: mcpConfigured ? trimmedKey : "",
      enabled: true,
      builtin: false,
      name: trimmedName,
      category,
      evidence: "custom",
      verified_at: null,
      docs: skillsConfigured ? trimmedSkillsDocs : null,
      skills: skillsConfigured ? {
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
        <span
          className="mux-agent-create-summary"
          data-state={incompleteCapabilities
            ? "incomplete"
            : hasCapability ? "ready" : "empty"}
          aria-live="polite"
        >
          <span />
          {incompleteCapabilities
            ? t("agents.capabilityIncompleteSummary", {
                capabilities: incompleteCapabilities,
              })
            : hasCapability
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
                    ? t(skillsStarted ? "agents.idInvalidWithSkills" : "agents.idInvalid")
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

          <div
            className="mux-agent-capability-tabs"
            role="tablist"
            aria-label={t("agents.capabilitiesTitle")}
            onKeyDown={handleCapabilityTabKeyDown}
          >
            <button
              ref={mcpTabRef}
              type="button"
              role="tab"
              id="agent-capability-tab-mcp"
              aria-selected={activeCapability === "mcp"}
              aria-controls="agent-capability-panel-mcp"
              tabIndex={activeCapability === "mcp" ? 0 : -1}
              className="mux-agent-capability-tab"
              data-active={activeCapability === "mcp" || undefined}
              data-state={mcpState}
              onClick={() => setActiveCapability("mcp")}
            >
              <span className="mux-agent-capability-tab-icon">
                <PackageIcon className="w-4 h-4" />
              </span>
              <span className="mux-agent-capability-tab-copy">
                <strong>MCP</strong>
                <small>{t(`agents.capabilityState.${mcpState}`)}</small>
              </span>
              <i aria-hidden="true" />
            </button>
            <button
              ref={skillsTabRef}
              type="button"
              role="tab"
              id="agent-capability-tab-skills"
              aria-selected={activeCapability === "skills"}
              aria-controls="agent-capability-panel-skills"
              tabIndex={activeCapability === "skills" ? 0 : -1}
              className="mux-agent-capability-tab"
              data-active={activeCapability === "skills" || undefined}
              data-state={skillsState}
              onClick={() => setActiveCapability("skills")}
            >
              <span className="mux-agent-capability-tab-icon">
                <SparklesIcon className="w-4 h-4" />
              </span>
              <span className="mux-agent-capability-tab-copy">
                <strong>Skills</strong>
                <small>{t(`agents.capabilityState.${skillsState}`)}</small>
              </span>
              <i aria-hidden="true" />
            </button>
          </div>

          {activeCapability === "mcp" ? (
            <section
              id="agent-capability-panel-mcp"
              role="tabpanel"
              aria-labelledby="agent-capability-tab-mcp"
              className="mux-agent-capability-detail"
            >
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
          ) : (
            <section
              id="agent-capability-panel-skills"
              role="tabpanel"
              aria-labelledby="agent-capability-tab-skills"
              className="mux-agent-capability-detail"
            >
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
