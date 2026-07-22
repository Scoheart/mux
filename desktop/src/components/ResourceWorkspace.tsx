import {
  createContext,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from "react";
import { AgentGlyph, agentName } from "./brandIcons";
import { MODAL_DIALOG_SELECTOR, Modal, SearchBar, wasHandledByLayer } from "./ui";
import { XIcon } from "./icons";
import {
  MAX_SIDEBAR_WIDTH,
  MIN_SIDEBAR_WIDTH,
  SIDEBAR_WIDTH_STORAGE_KEY,
  clampSidebarWidth,
  parseSidebarWidth,
} from "../lib/resourceWorkspace";

interface SidebarResizeSession {
  previousCursor: string;
  previousUserSelect: string;
  onPointerMove: (event: PointerEvent) => void;
  onPointerUp: () => void;
}

interface WorkspaceResizeContextValue {
  isResizing: boolean;
  sidebarWidth: number;
  onPointerDown: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onKeyDown: (event: ReactKeyboardEvent<HTMLDivElement>) => void;
}

interface ResourcePanelContextValue {
  panelId: string;
  setActiveTabId: (tabId: string | undefined) => void;
}

const WorkspaceResizeContext = createContext<WorkspaceResizeContextValue | null>(null);
const ResourcePanelContext = createContext<ResourcePanelContextValue | null>(null);

export interface ResourceTabOption<T extends string> {
  value: T;
  label: string;
  count: number;
}

export function ResourceTabs<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: ResourceTabOption<T>[];
  onChange: (value: T) => void;
}) {
  const tabListId = useId();
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const panel = useContext(ResourcePanelContext);
  const selectedIndex = Math.max(0, options.findIndex((option) => option.value === value));
  const selectedTabId = `${tabListId}-${selectedIndex}`;

  useEffect(() => {
    panel?.setActiveTabId(selectedTabId);
  }, [panel, selectedTabId]);

  useEffect(() => () => panel?.setActiveTabId(undefined), [panel]);

  const moveFocus = (index: number) => {
    const option = options[index];
    if (!option) return;
    onChange(option.value);
    tabRefs.current[index]?.focus();
  };

  return (
    <div className="mux-resource-tabs" role="tablist" aria-label={label}>
      {options.map((option, index) => {
        const tabId = `${tabListId}-${index}`;
        return (
          <button
            key={option.value}
            ref={(element) => {
              tabRefs.current[index] = element;
            }}
            type="button"
            className="mux-resource-tab"
            role="tab"
            id={tabId}
            aria-controls={panel?.panelId}
            aria-selected={option.value === value}
            tabIndex={option.value === value ? 0 : -1}
            data-active={option.value === value ? "true" : undefined}
            onClick={() => onChange(option.value)}
            onKeyDown={(event) => {
              if (options.length === 0) return;

              let nextIndex: number | null = null;
              if (event.key === "ArrowLeft") {
                nextIndex = (selectedIndex - 1 + options.length) % options.length;
              } else if (event.key === "ArrowRight") {
                nextIndex = (selectedIndex + 1) % options.length;
              } else if (event.key === "Home") {
                nextIndex = 0;
              } else if (event.key === "End") {
                nextIndex = options.length - 1;
              }

              if (nextIndex === null) return;
              event.preventDefault();
              moveFocus(nextIndex);
            }}
          >
            <span>{option.label}</span>
            <span className="mux-resource-tab-count">{option.count}</span>
          </button>
        );
      })}
    </div>
  );
}

export function ResourceWorkspace({
  sidebar,
  title,
  description,
  query,
  onQueryChange,
  searchPlaceholder,
  toolbarActions,
  children,
  filters,
  inspector,
  onInspectorClose,
}: {
  sidebar: ReactNode;
  title?: string;
  description?: string;
  query: string;
  onQueryChange: (value: string) => void;
  searchPlaceholder: string;
  toolbarActions: ReactNode;
  children: ReactNode;
  filters?: ReactNode;
  inspector?: ReactNode;
  onInspectorClose?: () => void;
}) {
  const [sidebarWidth, setSidebarWidth] = useState(() =>
    parseSidebarWidth(localStorage.getItem(SIDEBAR_WIDTH_STORAGE_KEY))
  );
  const sidebarWidthRef = useRef(sidebarWidth);
  const resizeSessionRef = useRef<SidebarResizeSession | null>(null);
  const [isResizing, setIsResizing] = useState(false);
  const workspaceId = useId();
  const panelId = `mux-resource-workspace-panel-${workspaceId}`;
  const [activeTabId, setActiveTabId] = useState<string>();
  const resourcePanelContext = useMemo(
    () => ({ panelId, setActiveTabId }),
    [panelId, setActiveTabId]
  );
  const isInspectorOpen = Boolean(inspector);

  const persistSidebarWidth = useCallback((width: number) => {
    localStorage.setItem(SIDEBAR_WIDTH_STORAGE_KEY, String(width));
  }, []);

  const updateSidebarWidth = useCallback((width: number) => {
    const nextWidth = clampSidebarWidth(width);
    sidebarWidthRef.current = nextWidth;
    setSidebarWidth(nextWidth);
    return nextWidth;
  }, []);

  const finishSidebarResize = useCallback(
    (persist: boolean) => {
      const session = resizeSessionRef.current;
      if (!session) return;

      resizeSessionRef.current = null;
      window.removeEventListener("pointermove", session.onPointerMove);
      window.removeEventListener("pointerup", session.onPointerUp);
      window.removeEventListener("pointercancel", session.onPointerUp);
      document.body.style.userSelect = session.previousUserSelect;
      document.body.style.cursor = session.previousCursor;
      setIsResizing(false);

      if (persist) persistSidebarWidth(sidebarWidthRef.current);
    },
    [persistSidebarWidth]
  );

  const onResizePointerDown = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (event.button !== 0) return;

      event.preventDefault();
      finishSidebarResize(true);
      const startWidth = sidebarWidthRef.current;
      const startX = event.clientX;
      const previousUserSelect = document.body.style.userSelect;
      const previousCursor = document.body.style.cursor;
      document.body.style.userSelect = "none";
      document.body.style.cursor = "col-resize";
      setIsResizing(true);

      const session: SidebarResizeSession = {
        previousCursor,
        previousUserSelect,
        onPointerMove: (moveEvent) => {
          updateSidebarWidth(startWidth + moveEvent.clientX - startX);
        },
        onPointerUp: () => finishSidebarResize(true),
      };

      resizeSessionRef.current = session;
      window.addEventListener("pointermove", session.onPointerMove);
      window.addEventListener("pointerup", session.onPointerUp);
      window.addEventListener("pointercancel", session.onPointerUp);
    },
    [finishSidebarResize, updateSidebarWidth]
  );

  const onResizeKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLDivElement>) => {
      let nextWidth: number | null = null;
      if (event.key === "ArrowLeft") nextWidth = sidebarWidthRef.current - 8;
      if (event.key === "ArrowRight") nextWidth = sidebarWidthRef.current + 8;
      if (event.key === "Home") nextWidth = MIN_SIDEBAR_WIDTH;
      if (event.key === "End") nextWidth = MAX_SIDEBAR_WIDTH;
      if (nextWidth === null) return;

      event.preventDefault();
      persistSidebarWidth(updateSidebarWidth(nextWidth));
    },
    [persistSidebarWidth, updateSidebarWidth]
  );

  useEffect(() => () => finishSidebarResize(true), [finishSidebarResize]);

  return (
    <ResourcePanelContext.Provider value={resourcePanelContext}>
      <WorkspaceResizeContext.Provider
        value={{
          isResizing,
          sidebarWidth,
          onPointerDown: onResizePointerDown,
          onKeyDown: onResizeKeyDown,
        }}
      >
        <div
          className="mux-workspace"
          style={{ "--mux-workspace-sidebar-width": `${sidebarWidth}px` } as CSSProperties}
        >
          {sidebar}
          <section className="mux-workspace-stage">
            {title && (
              <header className="mux-workspace-intro">
                <div>
                  <h1>{title}</h1>
                  {description && <p>{description}</p>}
                </div>
              </header>
            )}
            <div className="mux-workspace-toolbar">
              <SearchBar value={query} onChange={onQueryChange} placeholder={searchPlaceholder} />
              <div className="mux-workspace-actions">{toolbarActions}</div>
            </div>
            {filters && <div className="mux-workspace-filters">{filters}</div>}
            <div className="mux-workspace-content">
              <div
                id={panelId}
                className="mux-workspace-scroll"
                role="tabpanel"
                aria-label={searchPlaceholder}
                aria-labelledby={activeTabId}
                aria-hidden={isInspectorOpen || undefined}
                inert={isInspectorOpen || undefined}
              >
                {children}
              </div>
              {inspector && (
                <Modal
                  width="min(720px, calc(100vw - 32px))"
                  maxHeight="calc(100vh - 32px)"
                  ariaLabel="资源详情"
                  layer="detail"
                  onClose={() => onInspectorClose?.()}
                >
                  <div className="mux-workspace-inspector-surface">{inspector}</div>
                </Modal>
              )}
            </div>
          </section>
        </div>
      </WorkspaceResizeContext.Provider>
    </ResourcePanelContext.Provider>
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
  const resize = useContext(WorkspaceResizeContext);

  return (
    <aside className="mux-workspace-sidebar">
      <div className="mux-workspace-sidebar-head">
        <strong>{title}</strong>
        <span>{count} 项</span>
      </div>
      <div className="mux-workspace-sidebar-scroll">{children}</div>
      {resize && (
        <div
          className="mux-workspace-sidebar-resize"
          role="separator"
          aria-orientation="vertical"
          aria-label="调整侧边栏宽度"
          aria-valuemin={MIN_SIDEBAR_WIDTH}
          aria-valuemax={MAX_SIDEBAR_WIDTH}
          aria-valuenow={resize.sidebarWidth}
          tabIndex={0}
          data-resizing={resize.isResizing ? "true" : undefined}
          onPointerDown={resize.onPointerDown}
          onKeyDown={resize.onKeyDown}
        />
      )}
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
      if (event.key !== "Escape") return;
      if (wasHandledByLayer(event) || document.querySelector(MODAL_DIALOG_SELECTOR)) return;
      onClose();
    };
    document.addEventListener("keydown", closeOnEscape);
    return () => document.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  return (
    <aside
      className="mux-resource-inspector"
      data-resource-inspector
      data-modal-initial-focus
      tabIndex={-1}
      aria-label={`${title} 详情`}
    >
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
