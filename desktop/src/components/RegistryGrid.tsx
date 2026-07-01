import { useEffect, useMemo, useState } from "react";
import { listRegistry, scanInstalled } from "../lib/api";
import type { RegistryEntry, InstalledMcp } from "../lib/types";
import { SearchIcon, CheckIcon, PackageIcon } from "./icons";

export function RegistryGrid({ onPick }: { onPick: (e: RegistryEntry) => void }) {
  const [entries, setEntries] = useState<RegistryEntry[]>([]);
  const [installed, setInstalled] = useState<InstalledMcp[]>([]);
  const [q, setQ] = useState("");
  const [loading, setLoading] = useState(true);
  const [tagFilter, setTagFilter] = useState<string>("");

  useEffect(() => {
    Promise.all([
      listRegistry().then(setEntries).catch(console.error),
      scanInstalled().then(setInstalled).catch(console.error),
    ]).finally(() => setLoading(false));
  }, []);

  const countFor = (name: string) => installed.filter((i) => i.name === name).length;

  const allTags = useMemo(() => {
    const tagSet = new Set<string>();
    entries.forEach((e) => e.tags.forEach((t) => tagSet.add(t)));
    return Array.from(tagSet).sort();
  }, [entries]);

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    return entries.filter((e) => {
      const matchQ =
        !s ||
        e.name.toLowerCase().includes(s) ||
        e.description.toLowerCase().includes(s);
      const matchTag = !tagFilter || e.tags.includes(tagFilter);
      return matchQ && matchTag;
    });
  }, [entries, q, tagFilter]);

  return (
    <div>
      {/* Search */}
      <div className="relative mb-3">
        <SearchIcon className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 pointer-events-none"
          style={{ color: "var(--text-secondary)" }} />
        <input
          className="w-full pl-9 pr-4 py-2.5 text-sm rounded-mac border outline-none transition-shadow"
          style={{
            background: "var(--surface-raised)",
            border: "1px solid var(--border-hairline)",
            color: "var(--text-primary)",
          }}
          placeholder="搜索服务器…"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onFocus={(e) => {
            e.currentTarget.style.boxShadow = "0 0 0 3px rgba(0,122,255,.2)";
            e.currentTarget.style.borderColor = "#007AFF";
          }}
          onBlur={(e) => {
            e.currentTarget.style.boxShadow = "";
            e.currentTarget.style.borderColor = "var(--border-hairline)";
          }}
        />
      </div>

      {/* Tag pills */}
      {allTags.length > 0 && (
        <div className="flex flex-wrap gap-1.5 mb-4">
          <button
            onClick={() => setTagFilter("")}
            className="px-3 py-1 text-xs rounded-pill border transition-colors"
            style={{
              background: !tagFilter ? "#007AFF" : "var(--surface-raised)",
              color: !tagFilter ? "#fff" : "var(--text-secondary)",
              border: `1px solid ${!tagFilter ? "#007AFF" : "var(--border-hairline)"}`,
            }}
          >
            全部
          </button>
          {allTags.map((tag) => (
            <button
              key={tag}
              onClick={() => setTagFilter(tagFilter === tag ? "" : tag)}
              className="px-3 py-1 text-xs rounded-pill border transition-colors"
              style={{
                background: tagFilter === tag ? "#007AFF" : "var(--surface-raised)",
                color: tagFilter === tag ? "#fff" : "var(--text-secondary)",
                border: `1px solid ${tagFilter === tag ? "#007AFF" : "var(--border-hairline)"}`,
              }}
            >
              {tag}
            </button>
          ))}
        </div>
      )}

      {/* Grid */}
      <div className="grid gap-3" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))" }}>
        {loading ? (
          /* Skeleton cards */
          Array.from({ length: 6 }).map((_, i) => (
            <div
              key={i}
              className="rounded-mac p-4 animate-pulse"
              style={{
                background: "var(--surface-raised)",
                boxShadow: "var(--shadow-card)",
                minHeight: 120,
              }}
            >
              <div className="flex items-start gap-3 mb-3">
                <div className="w-9 h-9 rounded-mac flex-shrink-0"
                  style={{ background: "var(--border-hairline)" }} />
                <div className="flex-1 space-y-2 pt-0.5">
                  <div className="h-3 rounded" style={{ background: "var(--border-hairline)", width: "60%" }} />
                  <div className="h-2.5 rounded" style={{ background: "var(--border-hairline)", width: "40%" }} />
                </div>
              </div>
              <div className="space-y-1.5">
                <div className="h-2 rounded" style={{ background: "var(--border-hairline)", width: "100%" }} />
                <div className="h-2 rounded" style={{ background: "var(--border-hairline)", width: "80%" }} />
              </div>
            </div>
          ))
        ) : filtered.length === 0 ? (
          /* Empty state */
          <div className="col-span-full flex flex-col items-center justify-center py-16 gap-3"
            style={{ color: "var(--text-secondary)" }}>
            <PackageIcon className="w-12 h-12 opacity-30" />
            <p className="text-sm m-0">未找到匹配的服务器</p>
          </div>
        ) : (
          filtered.map((e) => {
            const c = countFor(e.name);
            const monogram = e.name[0]?.toUpperCase() ?? "?";
            return (
              <button
                key={e.name}
                onClick={() => onPick(e)}
                className="text-left rounded-mac p-4 transition-all cursor-pointer border-0 w-full"
                style={{
                  background: "var(--surface-raised)",
                  boxShadow: "var(--shadow-card)",
                }}
                onMouseEnter={(el) => {
                  el.currentTarget.style.transform = "translateY(-2px)";
                  el.currentTarget.style.boxShadow = "var(--shadow-hover)";
                }}
                onMouseLeave={(el) => {
                  el.currentTarget.style.transform = "";
                  el.currentTarget.style.boxShadow = "var(--shadow-card)";
                }}
              >
                {/* Header row */}
                <div className="flex items-start gap-3 mb-2">
                  {/* Monogram */}
                  <div
                    className="w-9 h-9 rounded-mac flex-shrink-0 flex items-center justify-center text-white text-sm font-semibold"
                    style={{
                      background: "linear-gradient(135deg, #007AFF, #5AC8FA)",
                    }}
                  >
                    {monogram}
                  </div>
                  <div className="flex-1 min-w-0 pt-0.5">
                    <div
                      className="text-sm font-semibold truncate"
                      style={{ color: "var(--text-primary)" }}
                    >
                      {e.name}
                    </div>
                    {/* Install badge */}
                    {c > 0 ? (
                      <div className="flex items-center gap-1 mt-0.5">
                        <CheckIcon className="w-3 h-3 text-green" />
                        <span className="text-[11px] text-green">已装 {c} 处</span>
                      </div>
                    ) : (
                      <div className="text-[11px] mt-0.5" style={{ color: "var(--text-secondary)" }}>
                        未安装
                      </div>
                    )}
                  </div>
                </div>

                {/* Description */}
                <p
                  className="text-xs m-0 mb-2 leading-relaxed"
                  style={{
                    color: "var(--text-secondary)",
                    display: "-webkit-box",
                    WebkitLineClamp: 2,
                    WebkitBoxOrient: "vertical",
                    overflow: "hidden",
                  }}
                >
                  {e.description}
                </p>

                {/* Tag pills */}
                {e.tags.length > 0 && (
                  <div className="flex flex-wrap gap-1">
                    {e.tags.slice(0, 3).map((tag) => (
                      <span
                        key={tag}
                        className="px-2 py-0.5 text-[10px] rounded-pill"
                        style={{
                          background: "color-mix(in srgb, #007AFF 10%, transparent)",
                          color: "#007AFF",
                        }}
                      >
                        {tag}
                      </span>
                    ))}
                  </div>
                )}
              </button>
            );
          })
        )}
      </div>
    </div>
  );
}
