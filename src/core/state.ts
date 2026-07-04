import type { StateConfig } from "../types.js";
import { mutateSettings } from "./settings.js";

export function writeState(state: StateConfig): void {
  mutateSettings((s) => {
    s.state = state;
  });
}
