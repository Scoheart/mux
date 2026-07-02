import { useMemo, useState } from "react";
import type { InstallState } from "../hooks/useInstallState";
import type { SourceView } from "../lib/types";
import { Avatar, Badge, IconButton, Switch } from "./ui";
import { CloudIcon, FolderIcon, LinkIcon, PlusIcon, RefreshIcon, TrashIcon, PackageIcon } from "./icons";
import { SubscribeDialog } from "./SubscribeDialog";
import { useToast } from "./Toast";

interface SourcesViewProps {
  state: InstallState;
}

/** The 来源 (Sources) page: manage subscribed remote URLs and local files. Each
 *  source contributes its servers to the aggregated Registry catalog. */
export function SourcesView({ state }: SourcesViewProps) {
  const { sources } = state;
  const toast = useToast();
  const [subscribeOpen, setSubscribeOpen] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);

  const sorted = useMemo(
    () =>
      [...sources].sort(
        (a, b) =>
          a.kind.localeCompare(b.kind) ||
          a.name.localeCompare(b.name, undefined, { sensitivity: "base" })
      ),
    [sources]
  );

  const totalServers = useMemo(
    () => sources.filter((s) => s.enabled).reduce((n, s) => n + s.server_count, 0),
    [sources]
  );

  const pickLocal = async () => {
    try {
      const v = await state.pickLocalSource();
      if (v) toast.show({ kind: "success", msg: `已添加本地来源：${v.name}（${v.server_count} 个 server）` });
    } catch (e) {
      toast.show({ kind: "error", msg: "添加失败：" + (Array.isArray(e) ? e.join("; ") : String(e)) });
    }
  };

  const addCurated = async () => {
    try {
      const v = await state.addCuratedCollection();
      toast.show({ kind: "success", msg: `已添加官方精选合集（${v.server_count} 个 server）` });
    } catch (e) {
      toast.show({ kind: "error", msg: "添加失败：" + (Array.isArray(e) ? e.join("; ") : String(e)) });
    }
  };

  const doRefresh = async (s: SourceView) => {
    setBusyId(s.id);
    try {
      await state.refreshOneSource(s.id);
      toast.show({ kind: "success", msg: `已刷新：${s.name}` });
    } catch (e) {
      toast.show({ kind: "error", msg: "刷新失败：" + (Array.isArray(e) ? e.join("; ") : String(e)) });
    } finally {
      setBusyId(null);
    }
  };

  const doToggle = async (s: SourceView, on: boolean) => {
    try {
      await state.toggleSource(s.id, on);
    } catch (e) {
      toast.show({ kind: "error", msg: "操作失败：" + (Array.isArray(e) ? e.join("; ") : String(e)) });
    }
  };

  const doRemove = async (s: SourceView) => {
    setBusyId(s.id);
    try {
      await state.deleteSource(s.id);
      toast.show({ kind: "success", msg: `已删除来源：${s.name}` });
    } catch (e) {
      toast.show({ kind: "error", msg: "删除失败：" + (Array.isArray(e) ? e.join("; ") : String(e)) });
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div className="h-full min-h-0 overflow-y-auto">
      {/* Sticky header: title + actions */}
      <div
        className="sticky top-0 z-10 px-6 py-4"
        style={{
          background: "var(--header-bg)",
          backdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          WebkitBackdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
          borderBottom: "1px solid color-mix(in srgb, var(--glass-border) 55%, transparent)",
        }}
      >
        <div className="max-w-[1280px] mx-auto flex items-center gap-3">
          <div className="flex-1 min-w-0">
            <h1 className="text-base font-semibold m-0" style={{ color: "var(--text-primary)" }}>
              来源
            </h1>
            <p className="text-xs m-0 mt-0.5" style={{ color: "var(--text-secondary)" }}>
              目录中的 MCP server 来自这些来源 · 共 {totalServers} 个（已启用）
            </p>
          </div>
          <button onClick={pickLocal} className="btn-ghost flex-shrink-0" title="从本地选择一个配置文件">
            <FolderIcon className="w-4 h-4" />
            添加本地文件
          </button>
          <button onClick={() => setSubscribeOpen(true)} className="btn-primary flex-shrink-0">
            <PlusIcon className="w-4 h-4" />
            订阅 URL
          </button>
        </div>
      </div>

      <div className="max-w-[1280px] mx-auto px-6 pt-5 pb-8">
        {sorted.length === 0 ? (
          <EmptyState onSubscribe={() => setSubscribeOpen(true)} onPickLocal={pickLocal} onCurated={addCurated} />
        ) : (
          <>
            <div className="mux-grid">
              {sorted.map((s) => (
                <SourceCard
                  key={s.id}
                  source={s}
                  busy={busyId === s.id}
                  onRefresh={() => doRefresh(s)}
                  onToggle={(on) => doToggle(s, on)}
                  onRemove={() => doRemove(s)}
                />
              ))}
            </div>
            <div className="mt-5 flex justify-center">
              <button onClick={addCurated} className="btn-ghost" title="把 MUX 内置的精选合集作为一个本地来源加入">
                <PackageIcon className="w-4 h-4" />
                添加官方精选合集
              </button>
            </div>
          </>
        )}
      </div>

      {subscribeOpen && <SubscribeDialog state={state} onClose={() => setSubscribeOpen(false)} />}
    </div>
  );
}

function SourceCard({
  source,
  busy,
  onRefresh,
  onToggle,
  onRemove,
}: {
  source: SourceView;
  busy: boolean;
  onRefresh: () => void;
  onToggle: (on: boolean) => void;
  onRemove: () => void;
}) {
  const isRemote = source.kind === "remote";
  const location = isRemote ? source.url ?? "" : source.path ?? "（内嵌）";
  return (
    <div className="mux-tile p-3.5" style={{ opacity: source.enabled ? 1 : 0.6 }}>
      {/* Header: avatar + name + kind badge */}
      <div className="flex items-center gap-2.5">
        <Avatar seed={source.name} size={34} />
        <div className="flex-1 min-w-0">
          <div className="text-sm font-semibold truncate" style={{ color: "var(--text-primary)" }} title={source.name}>
            {source.name}
          </div>
          <div className="flex items-center gap-1.5 mt-1">
            <Badge tone={isRemote ? "info" : "neutral"} icon={isRemote ? <CloudIcon className="w-3 h-3" /> : <FolderIcon className="w-3 h-3" />}>
              {isRemote ? "订阅" : "本地"}
            </Badge>
            <Badge tone={source.server_count > 0 ? "success" : "neutral"}>{source.server_count} 个 server</Badge>
          </div>
        </div>
        <Switch checked={source.enabled} onChange={onToggle} title={source.enabled ? "已启用" : "已停用"} />
      </div>

      {/* Location (url / path) */}
      <div className="flex items-center gap-1.5 mt-2.5 min-w-0">
        {isRemote ? (
          <LinkIcon className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "var(--color-blue)" }} />
        ) : (
          <FolderIcon className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "var(--text-secondary)" }} />
        )}
        <span
          className="text-[11px] truncate"
          style={{ color: isRemote ? "var(--color-blue)" : "var(--text-secondary)", fontFamily: "var(--font-mono)" }}
          title={location}
        >
          {location}
        </span>
      </div>

      {/* Error, if any */}
      {source.error && (
        <div className="text-[11px] mt-2 leading-snug" style={{ color: "#FF375F" }} title={source.error}>
          ⚠ {source.error}
        </div>
      )}

      {/* Footer: synced-at + actions */}
      <div className="flex items-center justify-between mt-3 pt-3" style={{ borderTop: "1px solid var(--border-hairline)" }}>
        <span className="text-[11px] truncate" style={{ color: "var(--text-secondary)" }}>
          {source.synced_at ? `同步于 ${source.synced_at.replace("T", " ")}` : "未同步"}
        </span>
        <div className="flex items-center gap-0.5">
          <IconButton title="刷新（重新抓取 / 重新读取）" onClick={onRefresh} disabled={busy}>
            <RefreshIcon className="w-4 h-4" style={busy ? { animation: "spin 0.8s linear infinite" } : undefined} />
          </IconButton>
          <IconButton title="删除来源" onClick={onRemove} disabled={busy}>
            <TrashIcon className="w-4 h-4" />
          </IconButton>
        </div>
      </div>
    </div>
  );
}

function EmptyState({
  onSubscribe,
  onPickLocal,
  onCurated,
}: {
  onSubscribe: () => void;
  onPickLocal: () => void;
  onCurated: () => void;
}) {
  return (
    <div className="flex flex-col items-center justify-center py-20 text-center">
      <div
        className="w-16 h-16 rounded-2xl flex items-center justify-center mb-4"
        style={{ background: "var(--surface-raised)", border: "1px solid var(--border-hairline)" }}
      >
        <CloudIcon className="w-8 h-8" style={{ color: "var(--text-secondary)" }} />
      </div>
      <h2 className="text-base font-semibold m-0 mb-1.5" style={{ color: "var(--text-primary)" }}>
        还没有任何来源
      </h2>
      <p className="text-sm m-0 mb-5 max-w-[420px] leading-relaxed" style={{ color: "var(--text-secondary)" }}>
        MUX 不再内置 MCP server。订阅一个远程配置 URL，或添加一个本地配置文件，其中的 server 就会出现在 Registry 目录里。
      </p>
      <div className="flex items-center gap-2">
        <button onClick={onSubscribe} className="btn-primary">
          <PlusIcon className="w-4 h-4" />
          订阅 URL
        </button>
        <button onClick={onPickLocal} className="btn-ghost">
          <FolderIcon className="w-4 h-4" />
          添加本地文件
        </button>
        <button onClick={onCurated} className="btn-ghost">
          <PackageIcon className="w-4 h-4" />
          官方精选合集
        </button>
      </div>
    </div>
  );
}
