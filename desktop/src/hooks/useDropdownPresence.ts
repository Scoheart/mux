import { useCallback, useEffect, useState } from "react";

/** Dropdown open/close presence with exit animation before unmount. */
export function useDropdownPresence(openMs = 160, closeMs = 160) {
  const [open, setOpen] = useState(false);
  const [mounted, setMounted] = useState(false);
  const [phase, setPhase] = useState<"open" | "closing">("open");

  const show = useCallback(() => {
    setMounted(true);
    setPhase("open");
    setOpen(true);
  }, []);

  const hide = useCallback(() => {
    setOpen(false);
    setPhase("closing");
  }, []);

  const toggle = useCallback(() => {
    if (open) hide();
    else show();
  }, [hide, open, show]);

  useEffect(() => {
    if (phase !== "closing") return;
    const timer = window.setTimeout(() => setMounted(false), closeMs);
    return () => window.clearTimeout(timer);
  }, [closeMs, phase]);

  // Keep openMs available for CSS custom property consumers if needed later.
  void openMs;

  return { open, mounted, phase, show, hide, toggle, setOpen: (next: boolean) => (next ? show() : hide()) };
}
