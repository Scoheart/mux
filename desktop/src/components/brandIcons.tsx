import iconAliases from "../assets/agents/aliases.json";

const iconModules = import.meta.glob("../assets/agents/*.{png,svg,webp}", {
  eager: true,
  query: "?url",
  import: "default",
}) as Record<string, string>;

const LOGOS = Object.fromEntries(
  Object.entries(iconModules).map(([path, url]) => [path.split("/").pop()!.replace(/\.[^.]+$/, ""), url])
) as Record<string, string>;
const ICON_ALIASES: Record<string, string> = iconAliases;

/** Logos that are complete app icons (own background + rounded corners), so they
 *  render edge-to-edge instead of as a mark centered on a white tile. */
const FULL_BLEED = new Set<string>([
  "boltai",
  "codebuddy-code",
  "factory-droid",
  "firebender",
  "hermes",
  "kilo-code",
  "kimi-code",
  "kiro",
  "lmstudio",
  "minimax-code",
  "openhands",
  "pi",
  "qoder",
  "qoder-cli",
  "qoderwork",
  "roo-code",
  "rovo-dev",
  "warp",
]);

const THEMED_MARKS = new Set<string>(["augment"]);
const WIDE_TILES: Record<string, string> = { crush: "#654cff" };

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
  "grok-build": { name: "Grok Build", color: "#111111" },
  "minimax-code": { name: "MiniMax Code", color: "#75B9EA" },
  qoder: { name: "Qoder Desktop", color: "#6E56CF" },
  "qoder-cli": { name: "Qoder CLI", color: "#6E56CF" },
  qoderwork: { name: "QoderWork", color: "#25D959" },
  devin: { name: "Devin", color: "#1F2937" },
  kiro: { name: "Kiro", color: "#7C3AED" },
  junie: { name: "Junie", color: "#E5484D" },
  "amazon-q": { name: "Amazon Q", color: "#FF9900" },
  opencode: { name: "OpenCode", color: "#1F2937" },
  "copilot-cli": { name: "Copilot CLI", color: "#24292E" },
  cline: { name: "Cline", color: "#2563EB" },
  continue: { name: "Continue", color: "#111827" },
  warp: { name: "Warp", color: "#00B4C6" },
  pi: { name: "Pi", color: "#8B5CF6" },
};

export function agentName(id: string, explicitName?: string): string {
  return explicitName || AGENT_META[id]?.name || id;
}

const FALLBACK_COLORS = ["#3568D4", "#16856B", "#B84A62", "#9A6618", "#5E55B8", "#277B91"];

function fallbackColor(id: string): string {
  let hash = 0;
  for (const char of id) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  return FALLBACK_COLORS[hash % FALLBACK_COLORS.length];
}

/**
 * Square brand badge for an agent: the real logo on a white tile when available,
 * otherwise a brand-coloured monogram.
 */
export function AgentGlyph({ id, name, size = 26 }: { id: string; name?: string; size?: number }) {
  const logo = LOGOS[ICON_ALIASES[id] ?? id];
  const meta = AGENT_META[id];
  const displayName = agentName(id, name);
  const radius = Math.round(size * 0.3);

  if (logo) {
    if (WIDE_TILES[id]) {
      return (
        <div
          className="flex items-center justify-center"
          style={{
            width: size,
            height: size,
            borderRadius: radius,
            background: WIDE_TILES[id],
            overflow: "hidden",
          }}
        >
          <img
            src={logo}
            alt={displayName}
            draggable={false}
            style={{ width: "100%", height: "100%", objectFit: "contain", display: "block" }}
          />
        </div>
      );
    }

    // App-icon logos (own background) fill the badge; mark-only logos sit on a white tile.
    if (FULL_BLEED.has(id)) {
      return (
        <img
          src={logo}
          alt={displayName}
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
          background: THEMED_MARKS.has(id) ? "var(--surface-app)" : "#fff",
          border: "1px solid var(--border-hairline)",
        }}
      >
        <img
          src={logo}
          alt={displayName}
          draggable={false}
          style={{ width: Math.round(size * 0.64), height: Math.round(size * 0.64), objectFit: "contain" }}
        />
      </div>
    );
  }

  const label = displayName[0]?.toUpperCase() ?? "?";
  return (
    <div
      className="flex items-center justify-center text-white font-semibold select-none"
      style={{
        width: size,
        height: size,
        borderRadius: radius,
        background: meta?.color ?? fallbackColor(id),
        fontSize: Math.round(size * 0.5),
      }}
    >
      {label}
    </div>
  );
}
