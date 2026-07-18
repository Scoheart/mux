import type { ReactNode } from "react";

export type ResourceStateKind = "loading" | "empty" | "no-match" | "read-error" | "recovery";

export function ResourceState({
  kind,
  icon,
  title,
  detail,
  action,
}: {
  kind: ResourceStateKind;
  icon?: ReactNode;
  title: string;
  detail?: ReactNode;
  action?: ReactNode;
}) {
  if (kind === "loading") {
    return (
      <div className="mux-resource-state" data-state-kind={kind} role="status" aria-label={title}>
        <div className="mux-resource-skeleton" aria-hidden="true">
          {Array.from({ length: 6 }, (_, index) => <span key={index} />)}
        </div>
        <span className="sr-only">{title}</span>
      </div>
    );
  }

  const role = kind === "read-error" || kind === "recovery" ? "alert" : "status";
  return (
    <div className="mux-resource-state" data-state-kind={kind} role={role}>
      {icon != null && <span className="mux-resource-state-icon">{icon}</span>}
      <strong>{title}</strong>
      {detail != null && <span className="mux-resource-state-detail">{detail}</span>}
      {action != null && <div className="mux-resource-state-action">{action}</div>}
    </div>
  );
}
