import pc from "picocolors";
import { removeRegistryEntry } from "../core/registry.js";

export function removeCommand(name: string): void {
  // A name may have a stdio and/or an http variant; clear whichever exist.
  const removed = ["stdio", "http"].reduce(
    (acc, t) => removeRegistryEntry(name, t as "stdio" | "http") || acc,
    false
  );
  if (!removed) {
    console.log(pc.red(`"${name}" not found in registry.`));
    return;
  }
  console.log(pc.green(`✓ ${name} removed from registry`));
}
