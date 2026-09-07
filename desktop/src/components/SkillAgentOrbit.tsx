import { useLayoutEffect, useMemo, useRef, useState, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import { AgentGlyph, agentName } from "./brandIcons";
import { Modal } from "./ui";

type Point = { x: number; y: number };
type Pose = Point & { scale: number; opacity: number; rotation: number };
type Size = { width: number; height: number; cardHeight: number };

function orbitLayout(size: Size, cardWidth: number, count: number) {
  const { width, height, cardHeight } = size;
  const cx = width / 2;
  const cy = height / 2;
  const rx = (width - cardWidth) / 2 - 20;
  const ry = (height - cardHeight) / 2 - 20;
  const ring = (n: number, radiusX: number, radiusY: number) =>
    Array.from({ length: n }, (_, i) => {
      const angle = Math.PI / 2 + i * Math.PI * 2 / n;
      return { x: cx + Math.cos(angle) * radiusX, y: cy + Math.sin(angle) * radiusY };
    });
  const fits = (points: Point[]) => points.every((point, i) => {
    if (Math.abs(point.x - cx) < cardWidth / 2 + 90
      && Math.abs(point.y - cy) < cardHeight / 2 + 52) return false;
    return points.slice(i + 1).every((other) =>
      Math.abs(point.x - other.x) >= cardWidth + 4
      || Math.abs(point.y - other.y) >= cardHeight + 4);
  });
  let points = ring(count, rx, ry);
  let circular = width >= 700 && height >= 480 && fits(points);
  if (!circular && width >= 700 && height >= 480 && count > 8) {
    // Twelve outer cards and four inner cards keep a 16-Agent hand readable at 900 × 600.
    const outerCount = Math.ceil(count * .75);
    points = [...ring(outerCount, rx, ry), ...ring(count - outerCount, rx * .47, ry * .5)];
    circular = fits(points);
  }
  if (circular) return {
    grid: false, contentHeight: height,
    points: points.map((point) => ({ x: point.x - cardWidth / 2, y: point.y - cardHeight / 2 })),
  };

  // Reflow crowded hands instead of shrinking names or hiding Agents behind one another.
  const columns = Math.max(1, Math.min(count, Math.floor((width - 28) / (cardWidth + 12))));
  const rows = Math.ceil(count / columns);
  const contentHeight = rows * (cardHeight + 12) - 12;
  const left = (width - 40 - columns * (cardWidth + 12) + 12) / 2;
  const top = Math.max(0, (height - 160 - contentHeight) / 2);
  return {
    grid: true, contentHeight: contentHeight + top,
    points: Array.from({ length: count }, (_, i) => ({
      x: left + (i % columns) * (cardWidth + 12),
      y: top + Math.floor(i / columns) * (cardHeight + 12),
    })),
  };
}

function transform(pose: Pose) {
  return `translate3d(${pose.x}px, ${pose.y}px, 0) rotate(${pose.rotation}deg) scale(${pose.scale})`;
}

function readPose(card: HTMLElement): Pose {
  const style = getComputedStyle(card);
  const matrix = new DOMMatrixReadOnly(style.transform === "none" ? undefined : style.transform);
  return { x: matrix.m41, y: matrix.m42, scale: Math.hypot(matrix.a, matrix.b),
    rotation: Math.atan2(matrix.b, matrix.a) * 180 / Math.PI, opacity: Number(style.opacity) };
}

/** The hand expands across the App canvas; Modal supplies only global focus/layer coordination. */
export function SkillAgentOrbit({ ids, names, skillName, sourceRef, onExit }: {
  ids: string[];
  names: ReadonlyMap<string, string>;
  skillName?: string;
  sourceRef: RefObject<HTMLDivElement | null>;
  onExit: (id?: string) => void;
}) {
  const { t } = useTranslation();
  const stageRef = useRef<HTMLDivElement>(null);
  const cardsLayerRef = useRef<HTMLDivElement>(null);
  const cardRefs = useRef(new Map<string, HTMLButtonElement>());
  const positioned = useRef(false);
  const motionsRef = useRef<Animation[]>([]);
  const nextAgent = useRef<string | undefined>(undefined);
  const exitRef = useRef(onExit);
  exitRef.current = onExit;
  const [closing, setClosing] = useState(false);
  const [size, setSize] = useState<Size>({ width: window.innerWidth, height: window.innerHeight, cardHeight: 104 });
  const cardWidth = size.width >= 1100 ? 144 : 128;
  const layout = useMemo(() => orbitLayout(size, cardWidth, ids.length), [size, cardWidth, ids.length]);

  useLayoutEffect(() => () => { motionsRef.current.forEach((animation) => animation.cancel()); }, []);

  useLayoutEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    const measure = () => {
      const cardHeight = Math.max(104, ...Array.from(cardRefs.current.values(), (card) => card.offsetHeight));
      const width = stage.clientWidth;
      const height = stage.clientHeight;
      setSize((old) => old.width === width && old.height === height && old.cardHeight === cardHeight
        ? old : { width, height, cardHeight });
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(stage);
    cardRefs.current.forEach((card) => observer.observe(card));
    return () => observer.disconnect();
  }, [ids, names, cardWidth]);

  useLayoutEffect(() => {
    const layer = cardsLayerRef.current;
    if (!layer) return;
    const bounds = layer.getBoundingClientRect();
    const sources = Array.from(sourceRef.current?.querySelectorAll<HTMLElement>(".mux-skill-agent-card") ?? []);
    const sourceRects = sources.map((source) => source.getBoundingClientRect());
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const first = !positioned.current;
    let active = true;
    const animations: Animation[] = [];
    // Read all current transforms before writing any destination styles.
    const current = ids.map((id) => {
      const card = cardRefs.current.get(id);
      return card && !first && card.style.transform ? readPose(card) : null;
    });
    // Keep an interrupted flight at its current position before starting the return trip.
    motionsRef.current.forEach((animation) => animation.cancel());
    ids.forEach((id, i) => {
      const card = cardRefs.current.get(id);
      if (!card) return;
      const sourceIndex = sources.findIndex((source) => source.dataset.agentId === id);
      const source = sourceRects[sourceIndex >= 0 ? sourceIndex : sourceRects.length - 1];
      const origin: Pose = {
        x: (source ? source.left + source.width / 2 : size.width / 2) - bounds.left - cardWidth / 2,
        y: (source ? source.top + source.height / 2 : size.height / 2) - bounds.top - card.offsetHeight / 2,
        scale: source ? source.width / cardWidth : .25,
        rotation: sourceIndex >= 0 ? (sourceIndex - 1.5) * 5 : 12,
        opacity: sourceIndex >= 0 ? 1 : 0,
      };
      const target: Pose = { ...layout.points[i], scale: 1, rotation: 0, opacity: 1 };
      const from = current[i] ?? origin;
      const to = closing ? origin : target;
      card.style.transform = transform(to);
      card.style.opacity = String(to.opacity);
      if (reduced) return;
      const lift = closing ? 12 : 26;
      const middle: Pose = {
        x: from.x + (to.x - from.x) * .6,
        y: from.y + (to.y - from.y) * .6 - lift,
        scale: from.scale + (to.scale - from.scale) * .6,
        rotation: from.rotation * .4, opacity: from.opacity + (to.opacity - from.opacity) * .8,
      };
      animations.push(card.animate([
        { transform: transform(from), opacity: from.opacity },
        { transform: transform(middle), opacity: middle.opacity, offset: .45 },
        { transform: transform(to), opacity: to.opacity },
      ], {
        duration: closing ? 340 : first ? 520 : 240,
        delay: first ? Math.min(i * 9, 144) : closing ? Math.min((ids.length - i - 1) * 4, 64) : 0,
        easing: "cubic-bezier(.2,.75,.2,1)", fill: "backwards",
      }));
    });
    motionsRef.current = animations;
    positioned.current = true;
    if (closing) void Promise.allSettled(animations.map((animation) => animation.finished)).then(() => {
      if (active) exitRef.current(nextAgent.current);
    });
    return () => { active = false; };
  }, [layout, closing, cardWidth, ids, sourceRef, size.width, size.height]);

  const close = (id?: string) => {
    if (closing) return;
    nextAgent.current = id;
    setClosing(true);
  };

  return <Modal presentation="canvas" layer="skill-agent-orbit" onClose={() => close()}
    ariaLabel={t("skillLibrary.allAgents")}>
    <div ref={stageRef} className="mux-skill-agent-orbit" data-layout={layout.grid ? "grid" : "ring"}
      data-closing={closing || undefined} onClick={(event) => {
        if (!(event.target instanceof Element) || !event.target.closest("button")) close();
      }}>
      <div className="mux-skill-agent-orbit-scrim" aria-hidden="true" />
      <div className="mux-skill-agent-orbit-heading">
        <h2 title={skillName}>{skillName || t("skillLibrary.allAgents")}</h2>
        <span>{t("skillLibrary.agentCount", { count: ids.length })}</span>
        <button type="button" data-modal-initial-focus onClick={() => close()}>
          {t("skillLibrary.collapseAgents")}
        </button>
      </div>
      <div className="mux-skill-agent-orbit-scroll">
        <div ref={cardsLayerRef} className="mux-skill-agent-orbit-cards" style={{ height: layout.contentHeight }}>
          {ids.map((id) => {
            const name = agentName(id, names.get(id));
            return <button key={id} type="button" className="mux-skill-agent-orbit-card"
              ref={(node) => { if (node) cardRefs.current.set(id, node); else cardRefs.current.delete(id); }}
              style={{ width: cardWidth }} aria-label={t("skillLibrary.openAgent", { name })}
              onClick={() => close(id)}>
              <span className="mux-skill-agent-orbit-tile">
                <AgentGlyph id={id} name={name} size={40} />
                <span className="mux-skill-agent-orbit-name">{name}</span>
              </span>
            </button>;
          })}
        </div>
      </div>
    </div>
  </Modal>;
}
