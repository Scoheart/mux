import { useState, type ReactNode } from "react";
import { formatError } from "../lib/format";
import { DialogShell } from "./DialogShell";

export function ReviewDialog({
  title,
  subtitle,
  children,
  confirmLabel,
  tone = "danger",
  onConfirm,
  onClose,
}: {
  title: string;
  subtitle?: ReactNode;
  children: ReactNode;
  confirmLabel: string;
  tone?: "danger" | "primary";
  onConfirm: () => Promise<unknown> | unknown;
  onClose: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const confirm = async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await onConfirm();
    } catch (reason) {
      setError(formatError(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <DialogShell
      kind="review"
      title={title}
      subtitle={subtitle}
      status={error ? <div className="mux-review-error" role="alert">{error}</div> : undefined}
      busy={busy}
      onClose={onClose}
      footerEnd={
        <>
          <button type="button" className="btn-ghost" disabled={busy} onClick={onClose}>取消</button>
          <button
            type="button"
            className={tone === "danger" ? "btn-danger" : "btn-primary"}
            disabled={busy}
            onClick={() => void confirm()}
          >
            {busy ? "处理中…" : confirmLabel}
          </button>
        </>
      }
    >
      <div className="mux-review-content">{children}</div>
    </DialogShell>
  );
}
