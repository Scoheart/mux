import { useSyncExternalStore } from "react";

// One invalidation clock for model queries in every mounted page. Form state
// stays local; observation and manual rescans do not remount the editor.
let revision = 0;
const listeners = new Set<() => void>();
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
};
const snapshot = () => revision;
export function useModelObservationRevision() {
  return useSyncExternalStore(subscribe, snapshot, snapshot);
}
export async function invalidateModelObservation() {
  revision += 1;
  for (const listener of listeners) listener();
}
