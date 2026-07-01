import { CSSProperties, ReactNode } from "react";
import { SearchIcon } from "./icons";

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

export function gradientFor(seed: string): string {
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
        className="w-full pl-9 pr-3 py-2 text-sm rounded-mac outline-none"
        style={{
          background: "var(--surface-raised)",
          border: "1px solid var(--border-hairline)",
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
