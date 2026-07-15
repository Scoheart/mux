import { CSSProperties, ReactNode } from "react";
import { SearchIcon } from "./icons";
import { transportLabel, transportOf } from "../lib/mcp";
import type { RegistryEntry } from "../lib/types";

/* ── Avatar ──────────────────────────────────────────────────────────── */

// A small palette of macOS-flavoured gradients; picked deterministically by name.
const GRADIENTS = [
  "linear-gradient(135deg, #007AFF, #5AC8FA)",
  "linear-gradient(135deg, #FF8C64, #E664C8)",
  "linear-gradient(135deg, #34C759, #30D158)",
  "linear-gradient(135deg, #FFC83C, #FF8C64)",
  "linear-gradient(135deg, #5E5CE6, #BF5AF2)",
  "linear-gradient(135deg, #FF375F, #FF8C64)",
  "linear-gradient(135deg, #64D2FF, #007AFF)",
];

function gradientFor(seed: string): string {
  let h = 0;
  for (let i = 0; i < seed.length; i++) h = (h * 31 + seed.charCodeAt(i)) >>> 0;
  return GRADIENTS[h % GRADIENTS.length];
}

export function Avatar({
  seed,
  label,
  mono,
  size = 40,
}: {
  seed: string;
  label?: string;
  mono?: boolean;
  size?: number;
}) {
  return (
    <div
      className="flex-shrink-0 flex items-center justify-center text-white font-semibold select-none"
      style={{
        width: size,
        height: size,
        borderRadius: Math.round(size * 0.3),
        fontSize: Math.round(size * 0.4),
        background: gradientFor(seed),
        fontFamily: mono ? "var(--font-mono)" : undefined,
      }}
    >
      {(label ?? seed)[0]?.toUpperCase() ?? "?"}
    </div>
  );
}

/* ── Badge ───────────────────────────────────────────────────────────── */

type Tone = "neutral" | "success" | "warning" | "info";

const TONE: Record<Tone, { bg: string; fg: string }> = {
  neutral: { bg: "var(--color-gray-150)", fg: "var(--color-gray-600)" },
  success: { bg: "color-mix(in srgb, #34C759 14%, transparent)", fg: "#1A9447" },
  warning: { bg: "color-mix(in srgb, #FF9500 14%, transparent)", fg: "#C26A00" },
  info: { bg: "color-mix(in srgb, #007AFF 12%, transparent)", fg: "#007AFF" },
};

export function Badge({
  tone = "neutral",
  children,
  icon,
}: {
  tone?: Tone;
  children: ReactNode;
  icon?: ReactNode;
}) {
  const c = TONE[tone];
  return (
    <span
      className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-xs font-medium whitespace-nowrap"
      style={{ background: c.bg, color: c.fg }}
    >
      {icon}
      {children}
    </span>
  );
}

/* ── IconButton ──────────────────────────────────────────────────────── */

export function IconButton({
  title,
  onClick,
  children,
  active,
  disabled,
}: {
  title: string;
  onClick?: (e: React.MouseEvent) => void;
  children: ReactNode;
  active?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      disabled={disabled}
      data-active={active ? "true" : undefined}
      className="mux-icon-btn"
      onClick={(e) => {
        e.stopPropagation();
        onClick?.(e);
      }}
    >
      {children}
    </button>
  );
}

/* ── Switch ──────────────────────────────────────────────────────────── */

/** A small macOS-style toggle. `checked` on → green; off → gray. */
export function Switch({
  checked,
  onChange,
  disabled,
  title,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
  title?: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      title={title}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        if (!disabled) onChange(!checked);
      }}
      className="relative flex-shrink-0 rounded-full border-0 transition-colors"
      style={{
        width: 36,
        height: 22,
        padding: 0,
        background: checked ? "#34C759" : "var(--color-gray-200)",
        cursor: disabled ? "default" : "pointer",
        opacity: disabled ? 0.5 : 1,
      }}
    >
      <span
        className="absolute rounded-full"
        style={{
          top: 2,
          left: checked ? 16 : 2,
          width: 18,
          height: 18,
          background: "#fff",
          boxShadow: "0 1px 2px rgba(0,0,0,0.25)",
          transition: "left 0.15s ease",
        }}
      />
    </button>
  );
}

/* ── SearchBar ───────────────────────────────────────────────────────── */

export function SearchBar({
  value,
  onChange,
  placeholder,
  style,
  autoFocus,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  style?: CSSProperties;
  autoFocus?: boolean;
}) {
  return (
    <div className="relative" style={style}>
      <SearchIcon
        className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 pointer-events-none"
        style={{ color: "var(--text-secondary)" }}
      />
      <input
        className="w-full pl-9 pr-3 py-2 text-sm outline-none"
        style={{
          background: "var(--surface-raised)",
          border: "1px solid var(--border-hairline)",
          borderRadius: "var(--radius-mac)",
          color: "var(--text-primary)",
        }}
        placeholder={placeholder}
        value={value}
        autoFocus={autoFocus}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}

/* ── Modal ───────────────────────────────────────────────────────────── */

/** Glass modal shell: dimmed blurred overlay + centered glass panel. Clicking
 *  the overlay closes; clicks inside are contained. */
export function Modal({
  width = 520,
  maxHeight = "82vh",
  onClose,
  children,
}: {
  width?: number;
  maxHeight?: string;
  onClose: () => void;
  children: ReactNode;
}) {
  return (
    <div
      className="fixed inset-0 flex items-center justify-center z-40"
      style={{ background: "rgba(0,0,0,.3)", backdropFilter: "blur(8px)", WebkitBackdropFilter: "blur(8px)" }}
      onClick={onClose}
    >
      <div
        className="flex flex-col rounded-mac-lg overflow-hidden"
        style={{
          width,
          maxHeight,
          background: "var(--surface-overlay)",
          backdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          WebkitBackdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          border: "1px solid var(--glass-border)",
          boxShadow: "var(--shadow-sheet), var(--glass-highlight)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {children}
      </div>
    </div>
  );
}

/** Standard form-dialog header: brand-gradient glyph tile + title + subtitle + ✕. */
export function ModalHeader({
  glyph,
  title,
  subtitle,
  onClose,
}: {
  glyph: ReactNode;
  title: string;
  subtitle: ReactNode;
  onClose: () => void;
}) {
  return (
    <div className="flex items-start gap-4 px-6 py-5" style={{ borderBottom: "1px solid var(--border-hairline)" }}>
      <div
        className="w-11 h-11 rounded-mac flex-shrink-0 flex items-center justify-center text-white text-2xl font-semibold leading-none"
        style={{ background: "linear-gradient(135deg, var(--color-brand-gold), var(--color-brand-coral), var(--color-brand-magenta))" }}
      >
        {glyph}
      </div>
      <div className="flex-1 min-w-0">
        <h2 className="text-base font-semibold m-0 mb-1" style={{ color: "var(--text-primary)" }}>
          {title}
        </h2>
        <p className="text-xs m-0 leading-relaxed" style={{ color: "var(--text-secondary)" }}>
          {subtitle}
        </p>
      </div>
      <button
        onClick={onClose}
        className="flex-shrink-0 w-7 h-7 rounded-full flex items-center justify-center border-0 cursor-pointer mt-0.5"
        style={{ background: "var(--border-hairline)", color: "var(--text-secondary)" }}
      >
        <span className="text-xs font-medium">✕</span>
      </button>
    </div>
  );
}

/* ── TransportPill ───────────────────────────────────────────────────── */

/** Gray mono pill showing an entry's transport (stdio / http / sse / custom).
 *  `compact` is the tighter list-row variant. */
export function TransportPill({ entry, compact }: { entry: RegistryEntry; compact?: boolean }) {
  const full = transportLabel(entry);
  // Long/custom http types (e.g. "streamable-http") collapse to the transport
  // bucket so the pill stays short and card meta rows don't wrap; the exact type
  // is kept in the tooltip.
  const label = full.length <= 5 ? full : transportOf(entry);
  return (
    <span
      title={full}
      className={`inline-flex items-center text-[10px] font-medium uppercase tracking-wide whitespace-nowrap flex-shrink-0 ${
        compact ? "px-1.5 py-0.5 rounded" : "px-2.5 py-1 rounded-full"
      }`}
      style={{ background: "var(--color-gray-150)", color: "var(--color-gray-600)", fontFamily: "var(--font-mono)" }}
    >
      {label}
    </span>
  );
}

/* ── Sticky glass header ─────────────────────────────────────────────── */

/** Shared style for the views' sticky glass page headers. */
export const stickyHeaderStyle: CSSProperties = {
  background: "var(--header-bg)",
  backdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
  WebkitBackdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
  borderBottom: "1px solid color-mix(in srgb, var(--glass-border) 55%, transparent)",
};
