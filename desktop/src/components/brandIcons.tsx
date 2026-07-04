import claudeLogo from "../assets/agents/claude.svg";
import openaiLogo from "../assets/agents/openai.svg";
import geminiLogo from "../assets/agents/gemini.svg";
import vscodeLogo from "../assets/agents/vscode.svg";
import copilotLogo from "../assets/agents/copilot.svg";
import zedLogo from "../assets/agents/zed.svg";
import windsurfLogo from "../assets/agents/windsurf.svg";
import cursorLogo from "../assets/agents/cursor.svg";
import opencodeLogo from "../assets/agents/opencode.svg";
import jetbrainsLogo from "../assets/agents/jetbrains.svg";
import amazonqLogo from "../assets/agents/amazon-q.svg";
import qoderLogo from "../assets/agents/qoder.svg";
import kiroLogo from "../assets/agents/kiro.svg";
import devinLogo from "../assets/agents/devin.svg";
import continueLogo from "../assets/agents/continue.png";
import clineLogo from "../assets/agents/cline.png";
import rooLogo from "../assets/agents/roo-code.png";

/** Real brand logos (svgl / simpleicons) per agent id. */
const LOGOS: Record<string, string> = {
  "claude-code": claudeLogo,
  "claude-desktop": claudeLogo,
  cursor: cursorLogo,
  vscode: vscodeLogo,
  codex: openaiLogo,
  zed: zedLogo,
  windsurf: windsurfLogo,
  gemini: geminiLogo,
  "amazon-q": amazonqLogo,
  "copilot-cli": copilotLogo,
  junie: jetbrainsLogo,
  opencode: opencodeLogo,
  qoder: qoderLogo,
  kiro: kiroLogo,
  devin: devinLogo,
  continue: continueLogo,
  cline: clineLogo,
  "roo-code": rooLogo,
};

/** Logos that are complete app icons (own background + rounded corners), so they
 *  render edge-to-edge instead of as a mark centered on a white tile. */
const FULL_BLEED = new Set<string>(["qoder", "kiro", "roo-code"]);

/** Human-readable product names + brand colour (colour used for the monogram fallback). */
const AGENT_META: Record<string, { name: string; color: string }> = {
  "claude-code": { name: "Claude Code", color: "#D97757" },
  "claude-desktop": { name: "Claude Desktop", color: "#C15F3C" },
  cursor: { name: "Cursor", color: "#111827" },
  vscode: { name: "VS Code", color: "#0A7ACA" },
  codex: { name: "Codex", color: "#10A37F" },
  zed: { name: "Zed", color: "#084CCF" },
  windsurf: { name: "Windsurf", color: "#09B6A2" },
  "roo-code": { name: "Roo Code", color: "#6C47FF" },
  gemini: { name: "Gemini", color: "#4285F4" },
  qoder: { name: "Qoder", color: "#6E56CF" },
  devin: { name: "Devin", color: "#1F2937" },
  kiro: { name: "Kiro", color: "#7C3AED" },
  junie: { name: "Junie", color: "#E5484D" },
  "amazon-q": { name: "Amazon Q", color: "#FF9900" },
  opencode: { name: "OpenCode", color: "#1F2937" },
  "copilot-cli": { name: "Copilot CLI", color: "#24292E" },
  cline: { name: "Cline", color: "#2563EB" },
  continue: { name: "Continue", color: "#111827" },
};

export function agentName(id: string): string {
  return AGENT_META[id]?.name ?? id;
}

/**
 * Square brand badge for an agent: the real logo on a white tile when available,
 * otherwise a brand-coloured monogram.
 */
export function AgentGlyph({ id, size = 26 }: { id: string; size?: number }) {
  const logo = LOGOS[id];
  const meta = AGENT_META[id];
  const radius = Math.round(size * 0.3);

  if (logo) {
    // App-icon logos (own background) fill the badge; mark-only logos sit on a white tile.
    if (FULL_BLEED.has(id)) {
      return (
        <img
          src={logo}
          alt={meta?.name ?? id}
          draggable={false}
          style={{ width: size, height: size, borderRadius: radius, objectFit: "cover", display: "block" }}
        />
      );
    }
    return (
      <div
        className="flex items-center justify-center"
        style={{
          width: size,
          height: size,
          borderRadius: radius,
          background: "#fff",
          border: "1px solid var(--border-hairline)",
        }}
      >
        <img
          src={logo}
          alt={meta?.name ?? id}
          draggable={false}
          style={{ width: Math.round(size * 0.64), height: Math.round(size * 0.64), objectFit: "contain" }}
        />
      </div>
    );
  }

  const label = (meta?.name ?? id)[0]?.toUpperCase() ?? "?";
  return (
    <div
      className="flex items-center justify-center text-white font-semibold select-none"
      style={{
        width: size,
        height: size,
        borderRadius: radius,
        background: meta?.color ?? "#8E8E93",
        fontSize: Math.round(size * 0.5),
      }}
    >
      {label}
    </div>
  );
}
