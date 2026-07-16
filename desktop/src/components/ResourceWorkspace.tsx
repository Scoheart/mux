import { ReactNode, useEffect } from "react";
import { AgentGlyph, agentName } from "./brandIcons";
import { SearchBar } from "./ui";
import { XIcon } from "./icons";

export function ResourceWorkspace({
  sidebar,
  query,
  onQueryChange,
  searchPlaceholder,
  toolbarActions,
  children,
  inspector,
}: {
  sidebar: ReactNode;
  query: string;
  onQueryChange: (value: string) => void;
  searchPlaceholder: string;
  toolbarActions: ReactNode;
  children: ReactNode;
  inspector?: ReactNode;
}) {
  return (
    <div className="mux-workspace">
      {sidebar}
      <section className="mux-workspace-stage">
        <div className="mux-workspace-toolbar">
          <SearchBar value={query} onChange={onQueryChange} placeholder={searchPlaceholder} />
          <div className="mux-workspace-actions">{toolbarActions}</div>
        </div>
        <div className="mux-workspace-content">
          <div className="mux-workspace-scroll">{children}</div>
          {inspector}
        </div>
      </section>
    </div>
  );
}

export function WorkspaceSidebar({
  title,
  count,
  children,
}: {
  title: string;
  count: number;
  children: ReactNode;
}) {
  return (
    <aside className="mux-workspace-sidebar">
      <div className="mux-workspace-sidebar-head">
        <strong>{title}</strong>
        <span>{count} 项</span>
      </div>
      <div className="mux-workspace-sidebar-scroll">{children}</div>
    </aside>
  );
}

export function SidebarSection({
  title,
  actions,
  children,
}: {
  title: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="mux-sidebar-section">
      <div className="mux-sidebar-section-head">
        <span>{title}</span>
        {actions && <div className="mux-sidebar-section-actions">{actions}</div>}
      </div>
      <div className="mux-sidebar-section-list">{children}</div>
    </section>
  );
}

export function SidebarItem({
  active,
  icon,
  label,
  count,
  actions,
  onClick,
}: {
  active: boolean;
  icon: ReactNode;
  label: string;
  count?: number;
  actions?: ReactNode;
  onClick: () => void;
}) {
  return (
    <div
      className="mux-sidebar-item group"
      data-active={active ? "true" : undefined}
      title={label}
      role="button"
      tabIndex={0}
      aria-pressed={active}
      onClick={onClick}
      onKeyDown={(event) => {
        if (event.target !== event.currentTarget) return;
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onClick();
        }
      }}
    >
      <span className="mux-sidebar-item-icon">{icon}</span>
      <span className="mux-sidebar-item-label">{label}</span>
      {typeof count === "number" && (
        <span className={`mux-sidebar-item-count ${actions ? "group-hover:hidden" : ""}`}>
          {count}
        </span>
      )}
      {actions && (
        <span
          className="mux-sidebar-item-actions hidden group-hover:flex"
          onClick={(event) => event.stopPropagation()}
        >
          {actions}
        </span>
      )}
    </div>
  );
}

export function ResourceGrid({ children }: { children: ReactNode }) {
  return <div className="mux-resource-grid">{children}</div>;
}

export function ResourceEmpty({
  icon,
  title,
  detail,
  action,
}: {
  icon?: ReactNode;
  title: string;
  detail?: string;
  action?: ReactNode;
}) {
  return (
    <div className="mux-resource-empty">
      {icon && <span className="mux-resource-empty-icon">{icon}</span>}
      <strong>{title}</strong>
      {detail && <span>{detail}</span>}
      {action}
    </div>
  );
}

export function ResourceInspector({
  title,
  subtitle,
  avatar,
  onClose,
  children,
  footer,
}: {
  title: string;
  subtitle?: ReactNode;
  avatar: ReactNode;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
}) {
  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !document.querySelector('[role="dialog"]')) onClose();
    };
    document.addEventListener("keydown", closeOnEscape);
    return () => document.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  return (
    <aside className="mux-resource-inspector" aria-label={`${title} 详情`}>
      <header className="mux-resource-inspector-head">
        {avatar}
        <div className="min-w-0 flex-1">
          <h2 title={title}>{title}</h2>
          {subtitle && <div className="mux-resource-inspector-subtitle">{subtitle}</div>}
        </div>
        <button type="button" className="mux-inspector-close" onClick={onClose} title="关闭" aria-label="关闭详情">
          <XIcon className="w-4 h-4" />
        </button>
      </header>
      <div className="mux-resource-inspector-body">{children}</div>
      {footer && <footer className="mux-resource-inspector-footer">{footer}</footer>}
    </aside>
  );
}

export function InspectorSection({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="mux-inspector-section">
      <h3>{title}</h3>
      {children}
    </section>
  );
}

export function InspectorField({
  label,
  children,
  mono,
}: {
  label: string;
  children: ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="mux-inspector-field">
      <span>{label}</span>
      <div className={mono ? "mux-inspector-field-mono" : undefined}>{children}</div>
    </div>
  );
}

export function AgentStack({ ids, max = 4 }: { ids: string[]; max?: number }) {
  if (ids.length === 0) return <span className="mux-resource-usage-empty">未连接</span>;
  const visible = ids.slice(0, max);
  return (
    <span className="mux-agent-stack" title={ids.map((id) => agentName(id)).join("、")}>
      <span className="mux-agent-stack-icons">
        {visible.map((id) => (
          <span className="mux-agent-stack-icon" key={id}>
            <AgentGlyph id={id} size={20} />
          </span>
        ))}
      </span>
      <span>{ids.length} 个 Agent</span>
    </span>
  );
}
