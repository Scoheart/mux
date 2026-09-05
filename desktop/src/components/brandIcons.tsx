import iconAliases from "../assets/agents/aliases.json";
import agentSurfaces from "../assets/agents/surfaces.json";
import type { ReactNode } from "react";

type AgentSurface = "cli" | "desktop" | "ide" | "web";

const iconModules = import.meta.glob("../assets/agents/*.{png,svg,webp}", {
  eager: true,
  query: "?url",
  import: "default",
}) as Record<string, string>;

const LOGOS = Object.fromEntries(
  Object.entries(iconModules).map(([path, url]) => [path.split("/").pop()!.replace(/\.[^.]+$/, ""), url])
) as Record<string, string>;
const ICON_ALIASES: Record<string, string> = iconAliases;
const SURFACE_VALUES = new Set<AgentSurface>(["cli", "desktop", "ide", "web"]);
const AGENT_SURFACES: Record<string, string> = agentSurfaces;

function resolvedLogoKey(id: string): string {
  return ICON_ALIASES[id] ?? id;
}

function declaredSurface(id: string): AgentSurface | null {
  const value = AGENT_SURFACES[id];
  return SURFACE_VALUES.has(value as AgentSurface) ? value as AgentSurface : null;
}

const COLLIDING_LOGO_KEYS = (() => {
  const surfacesByLogo = new Map<string, Set<AgentSurface>>();
  for (const id of Object.keys(AGENT_SURFACES)) {
    const surface = declaredSurface(id);
    const logoKey = resolvedLogoKey(id);
    if (!surface || !LOGOS[logoKey]) continue;
    const surfaces = surfacesByLogo.get(logoKey) ?? new Set<AgentSurface>();
    surfaces.add(surface);
    surfacesByLogo.set(logoKey, surfaces);
  }
  return new Set(
    [...surfacesByLogo.entries()]
      .filter(([, surfaces]) => surfaces.size > 1)
      .map(([logoKey]) => logoKey),
  );
})();

function visibleSurface(id: string): AgentSurface | null {
  const logoKey = resolvedLogoKey(id);
  if (!COLLIDING_LOGO_KEYS.has(logoKey)) return null;
  return declaredSurface(id);
}

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
  "qoder-desktop",
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
  qoder: { name: "Qoder IDE", color: "#6E56CF" },
  "qoder-desktop": { name: "Qoder Desktop", color: "#6E56CF" },
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

function surfaceBadgeSize(size: number): number {
  if (size <= 24) return 10;
  if (size <= 36) return 12;
  return 14;
}

function AgentSurfaceBadge({ surface, size }: { surface: AgentSurface; size: number }) {
  const badgeSize = surfaceBadgeSize(size);
  return (
    <span
      className="mux-agent-surface-badge"
      data-agent-surface={surface}
      aria-hidden="true"
      style={{ width: badgeSize, height: badgeSize }}
    >
      <svg viewBox="0 0 12 12" fill="none" focusable="false">
        {surface === "cli" && (
          <>
            <path d="M2.25 3.25 4.5 5.5 2.25 7.75" />
            <path d="M5.5 8h4" />
          </>
        )}
        {surface === "desktop" && (
          <>
            <rect x="1.5" y="2" width="9" height="6.75" rx="1.25" />
            <path d="M4 10h4" />
          </>
        )}
        {surface === "ide" && (
          <>
            <rect x="1.5" y="1.75" width="9" height="8.5" rx="1.25" />
            <path d="M5 2v8M6.75 4h2M6.75 6h2" />
          </>
        )}
        {surface === "web" && (
          <>
            <circle cx="6" cy="6" r="4.5" />
            <path d="M1.75 6h8.5M6 1.75c1.35 1.25 2 2.67 2 4.25S7.35 9 6 10.25C4.65 9 4 7.58 4 6s.65-3 2-4.25Z" />
          </>
        )}
      </svg>
    </span>
  );
}

/**
 * Square brand badge for an agent: the real logo on a white tile when available,
 * otherwise a brand-coloured monogram.
 */
export function AgentGlyph({ id, name, size = 26 }: { id: string; name?: string; size?: number }) {
  const logo = LOGOS[resolvedLogoKey(id)];
  const meta = AGENT_META[id];
  const displayName = agentName(id, name);
  const radius = Math.round(size * 0.3);
  let baseGlyph: ReactNode;

  if (logo) {
    if (WIDE_TILES[id]) {
      baseGlyph = (
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
    } else if (FULL_BLEED.has(id)) {
      // App-icon logos (own background) fill the badge; mark-only logos sit on a white tile.
      baseGlyph = (
        <img
          src={logo}
          alt={displayName}
          draggable={false}
          style={{ width: size, height: size, borderRadius: radius, objectFit: "cover", display: "block" }}
        />
      );
    } else {
      baseGlyph = (
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
  } else {
    const label = displayName[0]?.toUpperCase() ?? "?";
    baseGlyph = (
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

  const surface = logo ? visibleSurface(id) : null;
  return (
    <span className="mux-agent-glyph" data-agent-id={id} style={{ width: size, height: size }}>
      <span className="mux-agent-glyph-base" style={{ width: size, height: size }}>
        {baseGlyph}
      </span>
      {surface && <AgentSurfaceBadge surface={surface} size={size} />}
    </span>
  );
}
