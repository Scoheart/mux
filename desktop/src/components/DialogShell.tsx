import type { CSSProperties, ReactNode } from "react";
import { XIcon } from "./icons";
import { Modal } from "./ui";

export type DialogShellKind = "editor" | "picker" | "review";
export type DialogShellSize = "sm" | "md" | "wide" | "lg";

const SIZE_WIDTH: Record<DialogShellSize, number> = {
  sm: 440,
  md: 560,
  wide: 640,
  lg: 760,
};

export function DialogShell({
  kind,
  size = kind === "review" ? "sm" : kind === "picker" ? "md" : "lg",
  width,
  className,
  borderRadius,
  leading,
  title,
  subtitle,
  status,
  busy = false,
  closeLabel = "关闭",
  onClose,
  children,
  footerStart,
  footerEnd,
}: {
  kind: DialogShellKind;
  size?: DialogShellSize;
  width?: CSSProperties["width"];
  className?: string;
  borderRadius?: CSSProperties["borderRadius"];
  leading?: ReactNode;
  title: string;
  subtitle?: ReactNode;
  status?: ReactNode;
  busy?: boolean;
  closeLabel?: string;
  onClose: () => void;
  children: ReactNode;
  footerStart?: ReactNode;
  footerEnd?: ReactNode;
}) {
  const requestClose = () => {
    if (!busy) onClose();
  };

  return (
    <Modal
      width={width ?? `min(${SIZE_WIDTH[size]}px, calc(100vw - 32px))`}
      maxHeight="calc(100vh - 32px)"
      borderRadius={borderRadius}
      ariaLabel={title}
      layer={kind}
      onClose={requestClose}
    >
      <section
        className={["mux-dialog-shell", className].filter(Boolean).join(" ")}
        data-dialog-kind={kind}
        data-dialog-size={size}
        aria-busy={busy || undefined}
      >
        <header className="mux-dialog-shell-header">
          {leading != null && <div className="mux-dialog-shell-leading">{leading}</div>}
          <div className="mux-dialog-shell-heading">
            <h2 data-modal-title tabIndex={-1}>{title}</h2>
            {subtitle != null && <div className="mux-dialog-shell-subtitle">{subtitle}</div>}
          </div>
          <button
            type="button"
            className="mux-dialog-shell-close"
            onClick={requestClose}
            disabled={busy}
            aria-label={closeLabel}
            title={closeLabel}
          >
            <XIcon className="w-4 h-4" />
          </button>
        </header>
        {status != null && <div className="mux-dialog-shell-status">{status}</div>}
        <div className="mux-dialog-shell-body">{children}</div>
        {(footerStart != null || footerEnd != null) && (
          <footer className="mux-dialog-shell-footer">
            <div className="mux-dialog-shell-footer-start">{footerStart}</div>
            <div className="mux-dialog-shell-footer-end">{footerEnd}</div>
          </footer>
        )}
      </section>
    </Modal>
  );
}
