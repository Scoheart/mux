import type { StateConfig } from "../types.js";
import { loadSettings, mutateSettings } from "./settings.js";

export function readState(): StateConfig {
  return loadSettings().state ?? { active: [] };
}

export function writeState(state: StateConfig): void {
  mutateSettings((s) => {
    s.state = state;
  });
}
