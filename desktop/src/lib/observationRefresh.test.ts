import { describe, expect, it } from "vitest";
import {
  ALL_OBSERVATION_TASK_IDS,
  focusRefreshDue,
  taskIdsForObservation,
} from "./observationRefresh";

describe("observation refresh routing", () => {
  it("refreshes only the capability affected by a filesystem event", () => {
    expect(taskIdsForObservation({ domains: ["mcp"] })).toEqual([
      "agents",
      "agent-capabilities",
      "relationships",
    ]);
    expect(taskIdsForObservation({ domains: ["skill"] })).toEqual([
      "agents",
      "agent-capabilities",
      "relationships",
      "skills",
    ]);
    expect(taskIdsForObservation({ domains: ["model"] })).toEqual([
      "agents",
      "agent-capabilities",
      "relationships",
      "external-models",
    ]);
  });

  it("deduplicates mixed domains and treats central or legacy events as global", () => {
    expect(taskIdsForObservation({ domains: ["mcp", "skill"] })).toEqual([
      "agents",
      "agent-capabilities",
      "relationships",
      "skills",
    ]);
    expect(taskIdsForObservation({ domains: ["central"] })).toEqual(
      ALL_OBSERVATION_TASK_IDS,
    );
    expect(taskIdsForObservation(undefined)).toEqual(ALL_OBSERVATION_TASK_IDS);
  });

  it("throttles the focus fallback independently of filesystem events", () => {
    expect(focusRefreshDue(10_000, 39_999)).toBe(false);
    expect(focusRefreshDue(10_000, 40_000)).toBe(true);
  });
});
