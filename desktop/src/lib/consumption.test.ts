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
