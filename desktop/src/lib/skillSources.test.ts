import { describe, expect, it } from "vitest";
import { skillsInventoryFixture } from "../test/skillsFixtures";
import { groupSkillSources, skillConsumerAgents, skillSourceGroup } from "./skillSources";
import type { SkillInventoryItem } from "./types";

describe("Skill source navigation", () => {
  it("groups a repository across subpaths, refs and case without merging distinct repositories", () => {
    const base = skillsInventoryFixture().items[0];
    if (base.source?.kind !== "github") throw new Error("fixture");
    const rows = [base, { ...base, name: "second", source: { ...base.source, owner: "ACME", subpath: "other", requested_ref: "dev" } },
      { ...base, name: "third", source: { ...base.source, repo: "other" } }];
    const groups = groupSkillSources(rows);
    expect(groups.find((g) => g.id === "github:acme/skills")?.count).toBe(2);
    expect(groups).toHaveLength(2);
  });

  it("keeps same-name local roots separate and distinguishes archives from folders", () => {
    const base = skillsInventoryFixture().items[1];
    const items: SkillInventoryItem[] = [
      { ...base, source: { kind: "local", path: "/one/skills/", subpath: "a" } },
      { ...base, source: { kind: "local", path: "/two/skills", subpath: "b" } },
      { ...base, source: { kind: "archive", path: "/one/skills", subpath: "a" } },
    ];
    expect(groupSkillSources(items).map((g) => g.id)).toHaveLength(3);
    expect(groupSkillSources(items).filter((g) => g.category === "local").map((g) => g.label))
      .toEqual(["/one/skills", "/two/skills"]);
  });

  it("groups imported copies by their original parent and retains unknown provenance", () => {
    const base = skillsInventoryFixture().items[0];
    expect(skillSourceGroup({ ...base, source: { kind: "imported", original_path: "~/.claude/skills/a", backup_path: "unused" } }).id)
      .toBe("imported:~/.claude/skills");
    expect(skillSourceGroup({ ...base, source: null }).id).toBe("unknown");
  });
});

it("shows only installed Agents with observed readable targets, deduplicating shared paths", () => {
  const inventory = skillsInventoryFixture();
  const base = inventory.items[0];
  const target = (states: SkillInventoryItem["states"], ids: string[]): SkillInventoryItem => ({
    ...base, location: { kind: "agent_target", target_id: "agents-user", global_dir: "~/.agents/skills" },
    states, affected_agent_ids: ids,
  });
  inventory.items.push(target(["assigned"], ["codex", "cursor", "not-installed"]));
  inventory.items.push(target(["external"], ["cursor", "claude-code"]));
  inventory.items.push(target(["missing"], ["gemini"]));
  inventory.items.push(target(["broken_link"], ["opencode"]));
  inventory.items.push(target(["conflicting_link"], ["copilot-cli"]));
  expect(skillConsumerAgents(inventory).get(base.name)).toEqual(["claude-code", "codex", "cursor"]);
  expect(skillConsumerAgents({ ...inventory, items: [base] }).size).toBe(0);
});
