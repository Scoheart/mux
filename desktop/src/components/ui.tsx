import { CSSProperties, ReactNode, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { SearchIcon, XIcon } from "./icons";
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
  ariaLabel,
  compact = false,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
  title?: string;
  ariaLabel?: string;
  compact?: boolean;
}) {
  const width = compact ? 30 : 36;
  const height = compact ? 18 : 22;
  const knob = compact ? 14 : 18;
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      title={title}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        if (!disabled) onChange(!checked);
      }}
      className="mux-switch relative flex-shrink-0 rounded-full border-0 transition-colors"
      style={{
        width,
        height,
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
          left: checked ? width - knob - 2 : 2,
          width: knob,
          height: knob,
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
    <div className="mux-search" style={style}>
      <SearchIcon
        className="mux-search-icon"
      />
      <input
        type="search"
        placeholder={placeholder}
        value={value}
        autoFocus={autoFocus}
        onChange={(e) => onChange(e.target.value)}
      />
      <button
        type="button"
        className="mux-search-clear"
        data-visible={value ? "true" : undefined}
        disabled={!value}
        tabIndex={value ? 0 : -1}
        onClick={() => onChange("")}
        title="清除搜索"
        aria-label="清除搜索"
      >
        <XIcon className="w-3.5 h-3.5" />
      </button>
    </div>
  );
}

/* ── Modal ───────────────────────────────────────────────────────────── */

export const MODAL_DIALOG_SELECTOR = '[role="dialog"][aria-modal="true"]';

const handledLayerKeyboardEvents = new WeakSet<KeyboardEvent>();
const inertRootStates = new WeakMap<
  HTMLElement,
  {
    count: number;
    hadAttribute: boolean;
    propertyValue?: boolean;
  }
>();

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "summary",
  '[contenteditable="true"]',
  '[tabindex]:not([tabindex="-1"])',
].join(",");

function topmostModal(): HTMLElement | null {
  const dialogs = document.querySelectorAll<HTMLElement>(MODAL_DIALOG_SELECTOR);
  return dialogs.item(dialogs.length - 1);
}

function modalFocusableElements(dialog: HTMLElement): HTMLElement[] {
  return Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) =>
      !element.hidden &&
      element.tabIndex >= 0 &&
      !element.closest('[aria-hidden="true"]') &&
      !element.closest("[inert]"),
  );
}

function acquireRootInert(): () => void {
  const root = document.getElementById("root");
  if (!root) return () => undefined;

  let state = inertRootStates.get(root);
  if (!state) {
    state = {
      count: 0,
      hadAttribute: root.hasAttribute("inert"),
      propertyValue: "inert" in root ? root.inert : undefined,
    };
    inertRootStates.set(root, state);
  }
  state.count += 1;
  if ("inert" in root) root.inert = true;
  root.setAttribute("inert", "");

  return () => {
    const current = inertRootStates.get(root);
    if (!current) return;
    current.count -= 1;
    if (current.count > 0) return;

    if (typeof current.propertyValue === "boolean" && "inert" in root) {
      root.inert = current.propertyValue;
    }
    if (current.hadAttribute) root.setAttribute("inert", "");
    else root.removeAttribute("inert");
    inertRootStates.delete(root);
  };
}

/** Coordinate one Escape/Tab across independently mounted UI layers. */
export function claimLayerKeyboardEvent(event: KeyboardEvent): void {
  handledLayerKeyboardEvents.add(event);
}

export function wasHandledByLayer(event: KeyboardEvent): boolean {
  return handledLayerKeyboardEvents.has(event);
}

/** Portal-backed modal shell with explicit stack, focus, and inert semantics. */
export function Modal({
  width = 520,
  maxHeight = "82vh",
  ariaLabel = "对话框",
  layer,
  onClose,
  children,
}: {
  width?: CSSProperties["width"];
  maxHeight?: CSSProperties["maxHeight"];
  ariaLabel?: string;
  layer?: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;

    const activeElement = document.activeElement;
    const opener = activeElement instanceof HTMLElement && activeElement.isConnected
      ? activeElement
      : null;
    const releaseRootInert = acquireRootInert();
    const focusFrame = requestAnimationFrame(() => {
      const initialTarget =
        dialog.querySelector<HTMLElement>("[data-modal-initial-focus]") ??
        dialog.querySelector<HTMLElement>("[data-modal-title]") ??
        modalFocusableElements(dialog)[0] ??
        dialog;
      initialTarget.focus();
    });

    const handleKeyDown = (event: KeyboardEvent) => {
      if (handledLayerKeyboardEvents.has(event) || topmostModal() !== dialog) return;

      if (event.key === "Escape") {
        claimLayerKeyboardEvent(event);
        event.preventDefault();
        onCloseRef.current();
        return;
      }

      if (event.key !== "Tab") return;
      const focusable = modalFocusableElements(dialog);
      const active = document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
      const activeIndex = active ? focusable.indexOf(active) : -1;
      const shouldWrap = event.shiftKey
        ? activeIndex <= 0
        : activeIndex === -1 || activeIndex === focusable.length - 1;
      if (!shouldWrap) return;

      claimLayerKeyboardEvent(event);
      event.preventDefault();
      const next = event.shiftKey
        ? focusable.at(-1) ?? dialog
        : focusable[0] ?? dialog;
      next.focus();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      cancelAnimationFrame(focusFrame);
      document.removeEventListener("keydown", handleKeyDown);
      releaseRootInert();
      requestAnimationFrame(() => {
        if (!opener?.isConnected) return;
        const remainingModal = topmostModal();
        if (!remainingModal || remainingModal.contains(opener)) opener.focus();
      });
    };
  }, []);

  return createPortal(
    <div
      className="fixed inset-0 flex items-center justify-center z-40"
      data-modal-overlay="true"
      data-modal-layer={layer}
      style={{ background: "rgba(0,0,0,.3)", backdropFilter: "blur(8px)", WebkitBackdropFilter: "blur(8px)", zIndex: 700 }}
      onClick={(event) => {
        if (event.target === event.currentTarget) event.stopPropagation();
        if (
          event.target !== event.currentTarget ||
          topmostModal() !== dialogRef.current
        ) return;
        onCloseRef.current();
      }}
    >
      <div
        ref={dialogRef}
        className="flex flex-col rounded-mac-lg overflow-hidden"
        role="dialog"
        aria-modal="true"
        aria-label={ariaLabel}
        data-modal-layer={layer}
        tabIndex={-1}
        style={{
          width,
          maxHeight,
          background: "var(--surface-overlay)",
          backdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          WebkitBackdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          border: "1px solid var(--glass-border)",
          boxShadow: "var(--shadow-sheet), var(--glass-highlight)",
        }}
        onClick={(event) => event.stopPropagation()}
      >
        {children}
      </div>
    </div>,
    document.body,
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
        <h2
          className="text-base font-semibold m-0 mb-1"
          data-modal-title
          tabIndex={-1}
          style={{ color: "var(--text-primary)" }}
        >
          {title}
        </h2>
        <p className="text-xs m-0 leading-relaxed" style={{ color: "var(--text-secondary)" }}>
          {subtitle}
        </p>
      </div>
      <button
        type="button"
        onClick={onClose}
        title="关闭"
        aria-label="关闭"
        className="flex-shrink-0 w-7 h-7 rounded-full flex items-center justify-center border-0 cursor-pointer mt-0.5"
        style={{ background: "var(--border-hairline)", color: "var(--text-secondary)" }}
      >
        <XIcon className="w-3.5 h-3.5" />
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
