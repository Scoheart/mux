import { useSyncExternalStore } from "react";

let revision = 0;
const listeners = new Set<() => void>();
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
};
const snapshot = () => revision;
export const usePreferenceRevision = () => useSyncExternalStore(subscribe, snapshot, snapshot);
export async function invalidatePreferences() {
  revision += 1;
  for (const listener of listeners) listener();
}
