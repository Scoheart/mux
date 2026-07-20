import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { expect, it } from "vitest";

const read = (path: string) => readFile(resolve(process.cwd(), path), "utf8");

it("keeps top-level MCP, Model, and Skill workspaces asset-only", async () => {
  const sources = await Promise.all([
    read("src/components/RegistryView.tsx"),
    read("src/components/ModelsView.tsx"),
    read("src/components/SkillsView.tsx"),
    read("src/components/SkillInspector.tsx"),
  ]);

  for (const source of sources) {
    expect(source).not.toContain("AssetConsumerDialog");
    expect(source).not.toContain("planForAsset");
    expect(source).not.toContain("consumersForAsset");
    expect(source).not.toContain("管理 Agent");
  }
});

it("owns multi-Model consumption and current-model switching in the Agent workspace", async () => {
  const source = await read("src/components/AgentView.tsx");
  expect(source).toContain("planForAgent");
  expect(source).toContain("可保留多个并切换当前模型");
  expect(source).toContain("planModelEnabled");
  expect(source).toContain("planActiveModel");
  expect(source).toContain("设为当前");
});
