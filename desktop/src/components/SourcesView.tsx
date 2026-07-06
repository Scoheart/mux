import { useMemo, useState } from "react";
import type { InstallState } from "../hooks/useInstallState";
import type { SourceView } from "../lib/types";
import { Avatar, Badge, IconButton, Switch, stickyHeaderStyle } from "./ui";
import { CloudIcon, FolderIcon, LinkIcon, RefreshIcon, TrashIcon, PackageIcon } from "./icons";
import { SubscribeDialog, OFFICIAL_SOURCE } from "./SubscribeDialog";
import { useToast } from "./Toast";
import { formatError } from "../lib/format";

interface SourcesViewProps {
  state: InstallState;
}

type SubscribePreset = { url?: string; name?: string } | null;

/** The 来源 (Sources) page. The catalog is assembled from:
 *  - 订阅 (remote): a URL MUX fetches + caches (refreshable, follows upstream),
 *  - 本地 (local): a file imported from disk (refreshable re-read),
 *  - MUX 维护: the managed 手动添加 / 自动探索 local entries. */
export function SourcesView({ state }: SourcesViewProps) {
  const { sources } = state;
  const toast = useToast();
  const [subscribe, setSubscribe] = useState<SubscribePreset>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  const sorted = useMemo(
    () =>
      [...sources].sort(
        (a, b) =>
          // remote first, then user local, then managed (manual/discovered) last
          rank(a) - rank(b) || a.name.localeCompare(b.name, undefined, { sensitivity: "base" })
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
      if (v) toast.show({ kind: "success", msg: `已导入本地来源：${v.name}（${v.server_count} 个 server）` });
    } catch (e) {
      toast.show({ kind: "error", msg: "导入失败：" + formatError(e) });
    }
  };

  const doRefresh = async (s: SourceView) => {
    setBusyId(s.id);
    try {
      if (s.id === "discovered") {
        await state.rescanDiscovered();
        toast.show({ kind: "success", msg: "已重新探索各 Agent 配置" });
      } else {
        await state.refreshOneSource(s.id);
        toast.show({ kind: "success", msg: `已刷新：${s.name}` });
      }
    } catch (e) {
      toast.show({ kind: "error", msg: "刷新失败：" + formatError(e) });
    } finally {
      setBusyId(null);
    }
  };

  const doToggle = async (s: SourceView, on: boolean) => {
    try {
      await state.toggleSource(s.id, on);
    } catch (e) {
      toast.show({ kind: "error", msg: "操作失败：" + formatError(e) });
    }
  };

  const doRemove = async (s: SourceView) => {
    setBusyId(s.id);
    try {
      await state.deleteSource(s.id);
      toast.show({ kind: "success", msg: `已删除来源：${s.name}` });
    } catch (e) {
      toast.show({ kind: "error", msg: "删除失败：" + formatError(e) });
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div className="h-full min-h-0 overflow-y-auto">
      {/* Sticky header: title + actions + legend */}
      <div className="sticky top-0 z-10 px-6 py-4" style={stickyHeaderStyle}>
        <div className="max-w-[1280px] mx-auto flex items-center gap-3">
          <div className="flex-1 min-w-0">
            <h1 className="text-base font-semibold m-0" style={{ color: "var(--text-primary)" }}>
              来源
            </h1>
            <p className="text-xs m-0 mt-0.5" style={{ color: "var(--text-secondary)" }}>
              目录中的 MCP server 来自这些来源 · 共 {totalServers} 个（已启用）
            </p>
          </div>
          <button
            onClick={() => setSubscribe({ url: OFFICIAL_SOURCE.url, name: OFFICIAL_SOURCE.name })}
            className="btn-ghost flex-shrink-0"
            title="一键订阅官方精选合集（远程源）"
          >
            <PackageIcon className="w-4 h-4" />
            官方精选
          </button>
          <button onClick={pickLocal} className="btn-ghost flex-shrink-0" title="从本机选择一个配置文件导入">
            <FolderIcon className="w-4 h-4" />
            导入本地配置
          </button>
          <button onClick={() => setSubscribe({})} className="btn-primary flex-shrink-0" title="订阅一个远程配置 URL">
            <CloudIcon className="w-4 h-4" />
            订阅远程配置
          </button>
        </div>
        <div className="max-w-[1280px] mx-auto mt-2 text-[11px] leading-relaxed" style={{ color: "var(--text-secondary)" }}>
          <b style={{ color: "var(--color-blue)" }}>订阅</b> = 远程配置源（URL），MUX 抓取缓存、可刷新、随远端更新 ·{" "}
          <b>本地</b> = 从本机导入一份副本、可刷新重读 ·{" "}
          <b>MUX 维护</b> = 手动添加 / 自动探索的本地条目
        </div>
      </div>

      <div className="max-w-[1280px] mx-auto px-6 pt-5 pb-8">
        {sorted.length === 0 ? (
          <EmptyState
            onSubscribe={() => setSubscribe({})}
            onPickLocal={pickLocal}
            onOfficial={() => setSubscribe({ url: OFFICIAL_SOURCE.url, name: OFFICIAL_SOURCE.name })}
          />
        ) : (
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
        )}
      </div>

      {subscribe && (
        <SubscribeDialog
          state={state}
          initialUrl={subscribe.url}
          initialName={subscribe.name}
          onClose={() => setSubscribe(null)}
        />
      )}
    </div>
  );
}

/** Sort rank: remote (0) → user local (1) → managed (2). */
function rank(s: SourceView): number {
  if (s.kind === "remote") return 0;
  return s.managed ? 2 : 1;
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
  const isManual = source.managed && source.id === "manual";
  const isDiscovered = source.managed && source.id === "discovered";

  const location = isRemote
    ? source.url ?? ""
    : isManual
      ? "在目录里新建 / 粘贴 / 编辑的条目"
      : isDiscovered
        ? "从各 Agent 全局配置中自动探索到的条目"
        : source.path ?? "";

  const kindLabel = isRemote ? "订阅" : source.managed ? "MUX 维护" : "本地";
  const kindIcon = isRemote ? <CloudIcon className="w-3 h-3" /> : source.managed ? <PackageIcon className="w-3 h-3" /> : <FolderIcon className="w-3 h-3" />;

  return (
    <div className="mux-tile p-3.5" style={{ opacity: source.enabled ? 1 : 0.6 }}>
      {/* Header: avatar + name + kind badge + toggle */}
      <div className="flex items-center gap-2.5">
        <Avatar seed={source.name} size={34} />
        <div className="flex-1 min-w-0">
          <div className="text-sm font-semibold truncate" style={{ color: "var(--text-primary)" }} title={source.name}>
            {source.name}
          </div>
          <div className="flex items-center gap-1.5 mt-1">
            <Badge tone={isRemote ? "info" : "neutral"} icon={kindIcon}>{kindLabel}</Badge>
            <Badge tone={source.server_count > 0 ? "success" : "neutral"}>{source.server_count} 个 server</Badge>
          </div>
        </div>
        <Switch checked={source.enabled} onChange={onToggle} title={source.enabled ? "已启用" : "已停用"} />
      </div>

      {/* Location / description */}
      <div className="flex items-center gap-1.5 mt-2.5 min-w-0">
        {isRemote ? (
          <LinkIcon className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "var(--color-blue)" }} />
        ) : (
          <FolderIcon className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "var(--text-secondary)" }} />
        )}
        <span
          className="text-[11px] truncate"
          style={{ color: isRemote ? "var(--color-blue)" : "var(--text-secondary)", fontFamily: isRemote || (!source.managed) ? "var(--font-mono)" : undefined }}
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

      {/* Footer */}
      <div className="flex items-center justify-between mt-3 pt-3" style={{ borderTop: "1px solid var(--border-hairline)" }}>
        {source.managed ? (
          <>
            <span className="text-[11px]" style={{ color: "var(--text-secondary)" }}>
              {isDiscovered ? "自动扫描" : "手动维护"}
            </span>
            {isDiscovered ? (
              <IconButton title="重新探索（重新扫描各 Agent 配置）" onClick={onRefresh} disabled={busy}>
                <RefreshIcon className="w-4 h-4" style={busy ? { animation: "spin 0.8s linear infinite" } : undefined} />
              </IconButton>
            ) : (
              <span />
            )}
          </>
        ) : (
          <>
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
          </>
        )}
      </div>
    </div>
  );
}

function EmptyState({
  onSubscribe,
  onPickLocal,
  onOfficial,
}: {
  onSubscribe: () => void;
  onPickLocal: () => void;
  onOfficial: () => void;
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
      <p className="text-sm m-0 mb-5 max-w-[460px] leading-relaxed" style={{ color: "var(--text-secondary)" }}>
        MUX 不内置 MCP server。<b>订阅</b>一个远程配置 URL，<b>导入</b>一个本机配置文件，或一键订阅<b>官方精选合集</b>——
        其中的 server 就会出现在 Registry 目录里。
      </p>
      <div className="flex items-center gap-2">
        <button onClick={onSubscribe} className="btn-primary">
          <CloudIcon className="w-4 h-4" />
          订阅 URL
        </button>
        <button onClick={onPickLocal} className="btn-ghost">
          <FolderIcon className="w-4 h-4" />
          导入本地文件
        </button>
        <button onClick={onOfficial} className="btn-ghost">
          <PackageIcon className="w-4 h-4" />
          官方精选
        </button>
      </div>
    </div>
  );
}
