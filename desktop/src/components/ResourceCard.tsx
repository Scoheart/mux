import type { KeyboardEvent, MouseEvent, ReactNode } from "react";

export type ResourceCardAttention = "warning" | "danger" | "shadowed";
export type ResourceKind = "model" | "mcp" | "skill";

export function ResourceKindIcon({
  kind,
  children,
}: {
  kind: ResourceKind;
  children: ReactNode;
}) {
  return (
    <span className="mux-resource-kind-icon" data-kind={kind} aria-hidden="true">
      {children}
    </span>
  );
}

export function ResourceCard({
  className,
  identity,
  configuration,
  state,
  impact,
  selected = false,
  attention,
  ariaLabel,
  onOpen,
}: {
  className?: string;
  identity: ReactNode;
  configuration?: ReactNode;
  state?: ReactNode;
  impact?: ReactNode;
  selected?: boolean;
  attention?: ResourceCardAttention;
  ariaLabel: string;
  onOpen: () => void;
}) {
  const openFromKeyboard = (event: KeyboardEvent<HTMLElement>) => {
    if (event.target !== event.currentTarget) return;
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    onOpen();
  };

  const openFromPointer = (event: MouseEvent<HTMLElement>) => {
    if ((event.target as HTMLElement).closest("button, a, input, select, textarea, summary")) {
      return;
    }
    onOpen();
  };

  return (
    <article
      className={`mux-resource-card${className ? ` ${className}` : ""}`}
      role="button"
      tabIndex={0}
      aria-label={ariaLabel}
      aria-pressed={selected}
      data-selected={selected ? "true" : undefined}
      data-attention={attention}
      onClick={openFromPointer}
      onKeyDown={openFromKeyboard}
    >
      <header className="mux-resource-card-identity" data-resource-slot="identity">
        {identity}
      </header>
      {configuration != null && (
        <div className="mux-resource-card-configuration" data-resource-slot="configuration">
          {configuration}
        </div>
      )}
      {state != null && (
        <div className="mux-resource-card-state" data-resource-slot="state">
          {state}
        </div>
      )}
      {impact != null && (
        <footer className="mux-resource-card-impact" data-resource-slot="impact">
          {impact}
        </footer>
      )}
    </article>
  );
}
