import pc from "picocolors";
import { writeRegistryEntry, readRegistry, keyOf } from "../core/registry.js";
import type { RegistryEntry } from "../types.js";
import { createInterface } from "node:readline";

async function prompt(question: string): Promise<string> {
  const rl = createInterface({ input: process.stdin, output: process.stdout });
  return new Promise((resolve) => {
    rl.question(question, (answer) => {
      rl.close();
      resolve(answer.trim());
    });
  });
}

export async function addCommand(name: string): Promise<void> {
  // `add` only creates stdio (command-based) entries → key is `${name}::stdio`.
  const existing = new Set(readRegistry().map(keyOf));

  if (existing.has(`${name}::stdio`)) {
    console.log(pc.yellow(`"${name}" (stdio) already exists in registry.`));
    return;
  }

  const description = await prompt("Description: ");
  const tagsRaw = await prompt("Tags (comma-separated): ");
  const command = await prompt("Command (e.g. npx): ");
  const argsRaw = await prompt("Args (comma-separated): ");

  const entry: RegistryEntry = {
    name,
    description,
    tags: tagsRaw ? tagsRaw.split(",").map((t) => t.trim()) : [],
    config: {
      stdio: {
        command,
        args: argsRaw ? argsRaw.split(",").map((a) => a.trim()) : [],
      },
    },
    origin: { kind: "manual" },
  };

  writeRegistryEntry(entry);
  console.log(pc.green(`✓ ${name} added to registry`));
}
