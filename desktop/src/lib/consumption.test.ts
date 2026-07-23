import { describe, expect, it } from "vitest";
import { consumptionInventoryFixture } from "../test/consumptionFixtures";
import {
  consumptionsForAgent,
  consumersForAsset,
  externalForAgent,
} from "./consumption";

describe("consumption selectors", () => {
  it("keeps desired and external projections separate", () => {
    const inventory = consumptionInventoryFixture();
    expect(consumptionsForAgent(inventory, "claude-code", "mcp")).toHaveLength(1);
    expect(externalForAgent(inventory, "claude-code", "mcp")).toHaveLength(1);
  });

  it("never mixes Skill assets into Model selectors", () => {
    const inventory = consumptionInventoryFixture();
    inventory.consumptions.push({
      agent_id: "codex",
      asset: { domain: "model", profile_id: "claude-opus-4-7" },
      desired: true,
      observed: true,
      enabled: true,
      active: true,
      desired_active: true,
      status: "synced",
      reason: null,
      affected_agent_ids: ["codex"],
    });
    const rows = consumptionsForAgent(inventory, "codex", "model");
    expect(rows).toHaveLength(1);
    expect(rows.every((item) => item.asset.domain === "model")).toBe(true);
    expect(rows.some((item) => item.asset.domain === "skill")).toBe(false);
  });

  it("selects consumers without reinterpreting core status", () => {
    const consumers = consumersForAsset(consumptionInventoryFixture(), {
      domain: "skill",
      name: "review-changes",
    });
    expect(consumers.map((item) => [item.agent_id, item.status])).toEqual([
      ["codex", "drifted"],
    ]);
  });
});
