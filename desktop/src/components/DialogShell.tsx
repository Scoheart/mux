import type { ReactNode } from "react";
import { XIcon } from "./icons";
import { Modal } from "./ui";

export type DialogShellKind = "editor" | "picker" | "review";
export type DialogShellSize = "sm" | "md" | "lg";

const SIZE_WIDTH: Record<DialogShellSize, number> = {
  sm: 440,
  md: 560,
  lg: 760,
};

export function DialogShell({
  kind,
  size = kind === "review" ? "sm" : kind === "picker" ? "md" : "lg",
  title,
  subtitle,
  status,
  busy = false,
  onClose,
  children,
  footerStart,
  footerEnd,
}: {
  kind: DialogShellKind;
  size?: DialogShellSize;
  title: string;
  subtitle?: ReactNode;
  status?: ReactNode;
  busy?: boolean;
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
      width={`min(${SIZE_WIDTH[size]}px, calc(100vw - 32px))`}
      maxHeight="calc(100vh - 32px)"
      ariaLabel={title}
      layer={kind}
      onClose={requestClose}
    >
      <section
        className="mux-dialog-shell"
        data-dialog-kind={kind}
        data-dialog-size={size}
        aria-busy={busy || undefined}
      >
        <header className="mux-dialog-shell-header">
          <div className="mux-dialog-shell-heading">
            <h2 data-modal-title tabIndex={-1}>{title}</h2>
            {subtitle != null && <div className="mux-dialog-shell-subtitle">{subtitle}</div>}
          </div>
          <button
            type="button"
            className="mux-dialog-shell-close"
            onClick={requestClose}
            disabled={busy}
            aria-label="关闭"
            title="关闭"
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
