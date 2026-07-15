/**
 * Scroll-reveal: IntersectionObserver toggles `.is-visible` on `[data-reveal]`.
 * Pattern adapted from Buzzy (seo-feature__row.is-visible).
 */

let observer: IntersectionObserver | null = null;

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function initScrollReveal(): void {
  if (typeof window === "undefined") return;

  observer?.disconnect();
  observer = null;

  const nodes = Array.from(
    document.querySelectorAll<HTMLElement>("[data-reveal]:not(.is-visible)"),
  );

  if (nodes.length === 0) return;

  if (prefersReducedMotion()) {
    for (const el of nodes) el.classList.add("is-visible");
    return;
  }

  observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        entry.target.classList.add("is-visible");
        observer?.unobserve(entry.target);
      }
    },
    {
      threshold: 0.12,
      rootMargin: "0px 0px -10% 0px",
    },
  );

  for (const el of nodes) observer.observe(el);
}
