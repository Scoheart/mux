/** Shared proportions for Skills and the navigation Agent hand. */
export function agentHandMetrics(count: number, cardWidth = 76) {
  const ratio = cardWidth / 76;
  return {
    step: (count < 5 ? 55 : 38) * ratio,
    spread: Math.min(26, Math.max(0, count - 1) * 3.5),
    rise: count <= 1 ? 0 : Math.min(58, count * 4) * ratio,
  };
}
