import { useLayoutEffect, useMemo, useRef, useState, type RefObject } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { AgentGlyph, agentName } from "./brandIcons";
import { acquireRootInert, claimLayerKeyboardEvent, wasHandledByLayer } from "./ui";

type Point = { x: number; y: number };
type Pose = Point & { scale: number; opacity: number; rotation: number };
type Size = { width: number; height: number; cardHeight: number };

function orbitLayout(size: Size, cardWidth: number, count: number) {
  const { width, height, cardHeight } = size;
  const cx = width / 2;
  const cy = height / 2;
  const unit = Array.from({ length: count }, (_, i) => {
    const angle = -Math.PI / 2 + i * Math.PI * 2 / count;
    return { x: Math.cos(angle), y: Math.sin(angle) };
  });
  // Keep one true circle. Its radius follows the cards, not the App's edges.
  let radius = 104;
  unit.forEach((point, i) => {
    unit.slice(i + 1).forEach((other) => {
      const dx = Math.abs(point.x - other.x);
      const dy = Math.abs(point.y - other.y);
      radius = Math.max(radius, Math.min(
        dx > .001 ? (cardWidth + 6) / dx : Infinity,
        dy > .001 ? (cardHeight + 6) / dy : Infinity,
      ));
    });
  });
  const scale = Math.min(1, (width - 40) / (radius * 2 + cardWidth),
    (height - 40) / (radius * 2 + cardHeight));
  return {
    scale, contentHeight: height,
    points: unit.map((point) => ({
      x: cx + point.x * radius * scale - cardWidth / 2,
      y: cy + point.y * radius * scale - cardHeight / 2,
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

/** Floating cards with background input blocked until the return flight completes. */
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
  const navigating = useRef(false);
  const exitRef = useRef(onExit);
  exitRef.current = onExit;
  const [closing, setClosing] = useState(false);
  const [size, setSize] = useState<Size>({ width: window.innerWidth, height: window.innerHeight, cardHeight: 60 });
  const cardWidth = 60;
  const layout = useMemo(() => orbitLayout(size, cardWidth, ids.length), [size, cardWidth, ids.length]);

  useLayoutEffect(() => () => {
    motionsRef.current.forEach((animation) => animation.cancel());
    motionsRef.current = [];
    // StrictMode replays mount effects. The replay must deal the cards again,
    // not treat their cancelled destination styles as an already-open ring.
    positioned.current = false;
  }, []);

  useLayoutEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    const measure = () => {
      const cardHeight = Math.max(60, ...Array.from(cardRefs.current.values(), (card) => card.offsetHeight));
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
      const source = sourceRects[sourceRects.length - 1];
      const origin: Pose = {
        x: (source ? source.left + source.width / 2 : size.width / 2) - bounds.left - cardWidth / 2,
        y: (source ? source.top + source.height / 2 : size.height / 2) - bounds.top - card.offsetHeight / 2,
        scale: source ? source.width / cardWidth : .25,
        rotation: -14,
        opacity: 0,
      };
      const target: Pose = { ...layout.points[i],
        y: layout.points[i].y + (size.cardHeight - card.offsetHeight) / 2,
        scale: layout.scale, rotation: 0, opacity: 1 };
      // Every new card is dealt from the same pile, and stays hidden until its turn.
      const from = current[i] ?? { ...origin, rotation: -14, opacity: 0 };
      const to = closing ? origin : target;
      card.style.transform = transform(to);
      card.style.opacity = String(to.opacity);
      card.disabled = closing || (first && !reduced);
      if (reduced) return;
      const angle = Math.atan2(target.y + card.offsetHeight / 2 - size.height / 2,
        target.x + cardWidth / 2 - size.width / 2);
      const tangent = { x: -Math.sin(angle) * 58, y: Math.cos(angle) * 58 };
      // Deal and collect follow the same curve in opposite directions.
      const pileControl = { x: origin.x + (target.x - origin.x) * .22, y: origin.y - 48 };
      const ringControl = { x: target.x - tangent.x, y: target.y - tangent.y };
      const control1 = closing ? ringControl : pileControl;
      const control2 = closing ? pileControl : ringControl;
      const frames = Array.from({ length: 13 }, (_, step) => {
        const progress = step / 12;
        const q = 1 - progress;
        const pose: Pose = {
          x: q ** 3 * from.x + 3 * q * q * progress * control1.x
            + 3 * q * progress * progress * control2.x + progress ** 3 * to.x,
          y: q ** 3 * from.y + 3 * q * q * progress * control1.y
            + 3 * q * progress * progress * control2.y + progress ** 3 * to.y,
          scale: from.scale + (to.scale - from.scale) * progress,
          rotation: from.rotation * q + to.rotation * progress - Math.sin(progress * Math.PI) * 8,
          opacity: closing ? from.opacity * Math.min(1, (1 - progress) * 12)
            : first ? (step === 0 ? 0 : 1) : from.opacity + (to.opacity - from.opacity) * progress,
        };
        return { transform: transform(pose), opacity: pose.opacity, offset: progress };
      });
      const animation = card.animate(frames, {
        duration: closing ? 180 : first ? 320 : 240,
        delay: first ? i * 65 : closing ? (ids.length - i - 1) * 24 : 0,
        easing: "cubic-bezier(.22,.62,.3,1)", fill: "backwards",
      });
      animation.onfinish = () => {
        if (!active || closing) return;
        card.disabled = false;
        if (first && i === 0 && (stageRef.current === document.activeElement
          || sourceRef.current?.contains(document.activeElement))) {
          card.focus({ preventScroll: true });
        }
      };
      animations.push(animation);
    });
    motionsRef.current = animations;
    positioned.current = true;
    if (closing) void Promise.allSettled(animations.map((animation) => animation.finished)).then(() => {
      if (active) exitRef.current();
    });
    return () => { active = false; };
  }, [layout, closing, cardWidth, ids, sourceRef, size.width, size.height]);

  const close = () => {
    if (closing || navigating.current) return;
    setClosing(true);
  };

  const openAgent = (id: string) => {
    if (closing || navigating.current) return;
    navigating.current = true;
    // Choosing a destination must not wait for the decorative collection sequence.
    exitRef.current(id);
  };

  const closeRef = useRef(close);
  closeRef.current = close;
  useLayoutEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    const opener = sourceRef.current?.querySelector<HTMLElement>(".mux-skill-agent-more");
    const releaseRootInert = acquireRootInert();
    const frame = requestAnimationFrame(() => {
      const firstCard = Array.from(cardRefs.current.values()).find((card) => !card.disabled);
      (firstCard ?? stage).focus({ preventScroll: true });
    });
    const keydown = (event: KeyboardEvent) => {
      if (wasHandledByLayer(event) || !["Escape", "Tab"].includes(event.key)) return;
      claimLayerKeyboardEvent(event);
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") { closeRef.current(); return; }
      const available = Array.from(cardRefs.current.values()).filter((card) => !card.disabled);
      const index = available.findIndex((card) => card === document.activeElement);
      const next = event.shiftKey
        ? (index <= 0 ? available.length - 1 : index - 1)
        : (index + 1) % available.length;
      (available[next] ?? stage).focus({ preventScroll: true });
    };
    document.addEventListener("keydown", keydown, true);
    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener("keydown", keydown, true);
      releaseRootInert();
      requestAnimationFrame(() => {
        if (!navigating.current && opener?.isConnected && !opener.closest("[inert]")) {
          opener.focus({ preventScroll: true });
        }
      });
    };
  }, [sourceRef]);

  return createPortal(
    <div ref={stageRef} className="mux-skill-agent-orbit" role="dialog" aria-modal="true" tabIndex={-1}
      aria-label={[skillName, t("skillLibrary.allAgents")].filter(Boolean).join(" · ")}
      data-closing={closing || undefined}
      onPointerDown={(event) => { event.stopPropagation(); }}
      onKeyDown={(event) => { event.stopPropagation(); }}
      onClick={(event) => {
        event.stopPropagation();
        if (!(event.target instanceof Element) || !event.target.closest("button")) close();
      }}>
      <div className="mux-skill-agent-orbit-scroll">
        <div ref={cardsLayerRef} className="mux-skill-agent-orbit-cards" style={{ height: layout.contentHeight }}>
          {ids.map((id) => {
            const name = agentName(id, names.get(id));
            return <button key={id} type="button" className="mux-skill-agent-orbit-card"
              ref={(node) => { if (node) cardRefs.current.set(id, node); else cardRefs.current.delete(id); }}
              style={{ width: cardWidth }} title={name} aria-label={t("skillLibrary.openAgent", { name })}
              onClick={() => openAgent(id)}>
              <span className="mux-skill-agent-orbit-tile">
                <AgentGlyph id={id} name={name} size={26} />
                <span className="mux-skill-agent-orbit-name">{name}</span>
              </span>
            </button>;
          })}
        </div>
      </div>
    </div>,
    document.body,
  );
}
