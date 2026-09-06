import { useState, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { AgentGlyph, agentName } from "./brandIcons";
import { Modal, ModalHeader } from "./ui";

export function SkillAgentHand({ ids, names, onOpenAgent }: {
  ids: string[];
  names: ReadonlyMap<string, string>;
  onOpenAgent?: (id: string) => void;
}) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  if (!ids.length) return <span className="mux-skill-unassigned">{t("skillLibrary.unassigned")}</span>;
  const visible = ids.slice(0, 4);
  const remaining = ids.length - visible.length;
  const count = visible.length + (remaining ? 1 : 0);
  const cardStyle = (index: number): CSSProperties => ({
    "--card-index": index,
    "--card-rotation": `${(index - (count - 1) / 2) * 7}deg`,
    "--card-offset": `${Math.abs(index - (count - 1) / 2) * 1.5}px`,
  } as CSSProperties);
  const open = (id: string) => { setExpanded(false); onOpenAgent?.(id); };
  return <>
    <div className="mux-skill-agent-hand" role="group" aria-label={t("skillLibrary.agents")}
      style={{ "--card-count": count } as CSSProperties}>
      {visible.map((id, index) => {
        const name = agentName(id, names.get(id));
        return <button key={id} type="button" className="mux-skill-agent-card"
          style={cardStyle(index)} data-agent-name={name}
          aria-label={t("skillLibrary.openAgent", { name })} onClick={() => open(id)}>
          <span aria-hidden="true"><AgentGlyph id={id} name={name} size={26} /></span>
        </button>;
      })}
      {remaining > 0 && <button type="button" className="mux-skill-agent-card mux-skill-agent-more"
        style={cardStyle(visible.length)} data-agent-name={t("skillLibrary.allAgents")}
        aria-label={t("skillLibrary.moreAgents", { count: remaining })} onClick={() => setExpanded(true)}>
        +{remaining}
      </button>}
    </div>
    {expanded && <Modal width={360} onClose={() => setExpanded(false)} ariaLabel={t("skillLibrary.allAgents")}>
      <ModalHeader glyph={null} subtitle={null} title={t("skillLibrary.allAgents")} onClose={() => setExpanded(false)} />
      <div className="mux-skill-agent-picker">
        {ids.map((id) => <button type="button" key={id} onClick={() => open(id)}>
          <AgentGlyph id={id} name={names.get(id)} size={28} />
          <span>{agentName(id, names.get(id))}</span>
        </button>)}
      </div>
    </Modal>}
  </>;
}
