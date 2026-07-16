import { openUrl } from "@tauri-apps/plugin-opener";
import type { RegistryEntry } from "../lib/types";
import { AgentGlyph, agentName } from "./brandIcons";
import {
  CopyIcon,
  EditIcon,
  LinkIcon,
  XIcon,
  CloudIcon,
  FolderIcon,
  LayersIcon,
  TrashIcon,
} from "./icons";
import { Avatar, Badge, Modal, TransportPill } from "./ui";

/** Provenance indicator for catalog / detail surfaces. */
export function OriginTag({
  entry,
  installedAgents,
  sourceName,
}: {
  entry: RegistryEntry;
  installedAgents: string[];
  sourceName: (id: string) => string;
}) {
  const origin = entry.origin;
  if (origin?.kind === "remote") {
    return (
      <span className="inline-flex items-center gap-1 min-w-0" title={`订阅：${origin.source ? sourceName(origin.source) : ""}`}>
        <CloudIcon className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "var(--color-blue)" }} />
        <span className="text-[11px] truncate" style={{ color: "var(--text-secondary)" }}>
          {origin.source ? sourceName(origin.source) : "订阅"}
        </span>
      </span>
    );
  }
  if (origin?.kind === "local") {
    return (
      <span className="inline-flex items-center gap-1 min-w-0" title={`本地：${origin.source ? sourceName(origin.source) : ""}`}>
        <FolderIcon className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "var(--text-secondary)" }} />
        <span className="text-[11px] truncate" style={{ color: "var(--text-secondary)" }}>
          {origin.source ? sourceName(origin.source) : "本地"}
        </span>
      </span>
    );
  }
  if (origin?.kind === "manual") return <Badge tone="info">手动</Badge>;
  const agent = origin?.agent ?? installedAgents[0];
  if (agent) {
    return (
      <span className="inline-flex items-center gap-1 min-w-0" title={`来自 ${agentName(agent)}`}>
        <span className="flex-shrink-0 inline-flex"><AgentGlyph id={agent} size={16} /></span>
        <span className="text-[11px] truncate" style={{ color: "var(--text-secondary)" }}>
          {agentName(agent)}
        </span>
      </span>
    );
  }
  return <Badge tone="neutral">探索</Badge>;
}

/** Shared MCP detail modal (Registry cards + Agent installed rows). */
export function RegistryDetail({
  entry,
  overriddenBy,
  installedAgents,
  sourceName,
  onClose,
  onCopy,
  onEdit,
  onDelete,
}: {
  entry: RegistryEntry;
  overriddenBy?: string;
  installedAgents: string[];
  sourceName: (id: string) => string;
  onClose: () => void;
  onCopy: () => void;
  onEdit?: () => void;
  onDelete?: () => void;
}) {
  return (
    <Modal width={560} onClose={onClose}>
      <div className="flex items-center gap-3 px-6 py-5" style={{ borderBottom: "1px solid var(--border-hairline)" }}>
        <Avatar seed={entry.name} size={40} />
        <div className="flex-1 min-w-0">
          <h2 className="text-base font-semibold m-0 truncate" style={{ color: "var(--text-primary)" }}>
            {entry.name}
          </h2>
          <div className="flex items-center gap-1.5 mt-1">
            <TransportPill entry={entry} />
            <OriginTag entry={entry} installedAgents={installedAgents} sourceName={sourceName} />
          </div>
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="关闭详情"
          title="关闭"
          className="flex-shrink-0 w-7 h-7 rounded-full flex items-center justify-center border-0 cursor-pointer"
          style={{ background: "var(--border-hairline)", color: "var(--text-secondary)" }}
        >
          <XIcon className="w-3.5 h-3.5" />
        </button>
      </div>

      {overriddenBy && (
        <div className="px-6 pt-4">
          <div className="mux-detail-warning">
            <LayersIcon className="w-4 h-4 flex-shrink-0" />
            <div className="min-w-0">
              <div className="text-xs font-semibold">已被覆盖</div>
              <div className="text-[11px] mt-0.5 leading-relaxed">
                当前使用「{overriddenBy}」，此副本不参与安装。
              </div>
            </div>
          </div>
        </div>
      )}

      <div className="flex-1 overflow-y-auto px-6 py-4 space-y-4">
        {entry.description && (
          <p className="text-sm leading-relaxed m-0" style={{ color: "var(--text-secondary)" }}>
            {entry.description}
          </p>
        )}
        {entry.tags.length > 0 && (
          <div className="flex flex-wrap gap-1.5">
            {entry.tags.map((t) => (
              <Badge key={t} tone="info">{t}</Badge>
            ))}
          </div>
        )}
        {entry.repo && (
          <div>
            <label className="text-xs font-medium block mb-1.5" style={{ color: "var(--text-secondary)" }}>
              仓库 / 主页
            </label>
            <button
              onClick={() => openUrl(entry.repo!)}
              className="inline-flex items-center gap-1.5 text-sm border-0 bg-transparent cursor-pointer p-0 break-all text-left"
              style={{ color: "var(--color-blue)" }}
              title="在浏览器中打开"
            >
              <LinkIcon className="w-3.5 h-3.5 flex-shrink-0" />
              {entry.repo}
            </button>
          </div>
        )}
        <div>
          <label className="text-xs font-medium block mb-2" style={{ color: "var(--text-secondary)" }}>
            配置
          </label>
          <pre
            className="text-xs overflow-x-auto m-0 p-3 rounded-mac"
            style={{
              background: "var(--surface-app)",
              border: "1px solid var(--border-hairline)",
              fontFamily: "var(--font-mono)",
              color: "var(--text-primary)",
            }}
          >
            {JSON.stringify(entry.config, null, 2)}
          </pre>
        </div>
      </div>

      <div className="flex items-center gap-2 px-6 py-4" style={{ borderTop: "1px solid var(--border-hairline)" }}>
        {onDelete && (
          <button
            onClick={onDelete}
            className="flex items-center gap-1.5 px-3 py-2 text-sm rounded-mac border-0 cursor-pointer"
            style={{ background: "transparent", color: "#FF3B30" }}
            title="删除条目（并从所有 agent 卸载）"
          >
            <TrashIcon className="w-4 h-4" />
            删除
          </button>
        )}
        <div className="flex-1" />
        <button
          onClick={onCopy}
          className="flex items-center gap-1.5 px-4 py-2 text-sm rounded-mac cursor-pointer"
          style={{ background: "transparent", border: "1px solid var(--border-hairline)", color: "var(--text-primary)" }}
        >
          <CopyIcon className="w-4 h-4" />
          复制 JSON
        </button>
        {onEdit && (
          <button
            onClick={onEdit}
            className="flex items-center gap-1.5 px-4 py-2 text-sm rounded-mac border-0 cursor-pointer font-medium"
            style={{ background: "var(--color-blue)", color: "#fff" }}
          >
            <EditIcon className="w-4 h-4" />
            编辑
          </button>
        )}
      </div>
    </Modal>
  );
}
