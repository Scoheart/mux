import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { expect, it } from "vitest";

const productionOwners = [
  "src/components/AgentView.tsx",
  "src/components/RegistryView.tsx",
  "src/components/RegistryEditPage.tsx",
  "src/components/ModelsView.tsx",
  "src/components/SkillsView.tsx",
  "src/components/SkillInspector.tsx",
  "src/hooks/useInstallState.ts",
];

it("keeps Agent and central views off legacy direct mutation APIs", async () => {
  const source = (
    await Promise.all(
      productionOwners.map((path) => readFile(resolve(process.cwd(), path), "utf8")),
    )
  ).join("\n");
  for (const legacy of [
    "applyInstall",
    "uninstall",
    "disableMcp",
    "enableMcp",
    "deleteMcp",
    "applyModelProfile",
    "saveModelProfile",
    "deleteModelProfile",
    "upsertRegistry",
    "deleteRegistry",
    "resyncEntry",
    "forgetEntry",
    "planSkillAssignment",
  ]) {
    expect(source, legacy).not.toContain(legacy);
  }
});
