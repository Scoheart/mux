import { useMemo, useState } from "react";
import type { AssetRef, ConsumptionView } from "../lib/types";
import { AgentGlyph } from "./brandIcons";
import { DialogShell } from "./DialogShell";
import { SearchBar } from "./ui";
import { ConsumptionStatus } from "./ConsumptionStatus";

export interface AssetConsumerOption {
  id: string;
  name: string;
  description?: string;
  disabled?: boolean;
  reason?: string;
  /** Agents sharing one physical target are selected as an indivisible group. */
  affectedAgentIds?: string[];
}

export function AssetConsumerDialog({
  asset,
  assetName,
  options,
  consumers,
  onReview,
  onClose,
}: {
  asset: AssetRef;
  assetName: string;
  options: AssetConsumerOption[];
  consumers: ConsumptionView[];
  onReview(agentIds: string[]): Promise<unknown> | unknown;
  onClose(): void;
}) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(
    () => new Set(consumers.map((consumer) => consumer.agent_id)),
  );
  const [busy, setBusy] = useState(false);
  const statusByAgent = useMemo(
    () => new Map(consumers.map((consumer) => [consumer.agent_id, consumer])),
    [consumers],
  );
  const filtered = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    return options.filter((option) =>
      !needle || `${option.name} ${option.id} ${option.description ?? ""}`
        .toLocaleLowerCase()
        .includes(needle),
    );
  }, [options, query]);

  const toggle = (option: AssetConsumerOption) => {
    setSelected((current) => {
      const next = new Set(current);
      const group = option.affectedAgentIds?.length ? option.affectedAgentIds : [option.id];
      const remove = group.every((agentId) => next.has(agentId));
      for (const agentId of group) {
        if (remove) next.delete(agentId);
        else next.add(agentId);
      }
      return next;
    });
  };

  const review = async () => {
    setBusy(true);
    try {
      await onReview([...selected].sort());
    } finally {
      setBusy(false);
    }
  };

  const domainLabel = asset.domain === "mcp" ? "MCP" : asset.domain === "model" ? "Model" : "Skill";
  return (
    <DialogShell
      kind="picker"
      title="管理使用此资产的 Agent"
      subtitle={`${domainLabel} · ${assetName}。选择只表达 desired relationship，确认后先展示完整写入影响。`}
      busy={busy}
      onClose={onClose}
      footerStart={<span className="mux-picker-count">已选择 {selected.size} 个 Agent</span>}
      footerEnd={
        <>
          <button type="button" className="btn-ghost" disabled={busy} onClick={onClose}>取消</button>
          <button type="button" className="btn-primary" disabled={busy} onClick={() => void review()}>
            {busy ? "生成计划…" : "审阅变更"}
          </button>
        </>
      }
    >
      <div className="mux-picker-search">
        <SearchBar value={query} onChange={setQuery} placeholder="搜索 Agent" autoFocus />
      </div>
      <div className="mux-picker-list" role="group" aria-label={`为 ${assetName} 选择 Agent`}>
        {filtered.length === 0 ? (
          <div className="mux-picker-empty">没有兼容的 Agent</div>
        ) : filtered.map((option) => {
          const current = statusByAgent.get(option.id);
          const group = option.affectedAgentIds?.length ? option.affectedAgentIds : [option.id];
          const optionSelected = group.every((agentId) => selected.has(agentId));
          return (
            <button
              key={option.id}
              type="button"
              className="mux-picker-option mux-consumption-picker-option mux-asset-consumer-option"
              data-selected={optionSelected ? "true" : undefined}
              disabled={option.disabled || busy}
              aria-pressed={optionSelected}
              onClick={() => toggle(option)}
            >
              <AgentGlyph id={option.id} name={option.name} size={30} />
              <span className="mux-picker-option-copy">
                <strong>{option.name}</strong>
                <small>{option.description ?? option.id}</small>
                {group.length > 1 && <em>共享目标 · 同时影响 {group.length} 个 Agent</em>}
                {option.reason && <em>{option.reason}</em>}
              </span>
              {current && <ConsumptionStatus status={current.status} reason={current.reason} />}
              <span className="mux-consumption-picker-check" aria-hidden="true">
                {optionSelected ? "✓" : ""}
              </span>
            </button>
          );
        })}
      </div>
    </DialogShell>
  );
}
