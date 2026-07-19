import { describe, expect, it } from "vitest";
import type { ConsumptionStatus, ConsumptionView, ModelProfileView } from "./types";
import { describeAgentModel } from "./agentModels";

const profile: ModelProfileView = {
  id: "team-openai",
  name: "Team OpenAI",
  protocol: "openai-responses",
  base_url: "https://example.invalid/v1",
  model: "gpt-test",
  reasoning: false,
  credential_saved: true,
};

function consumption(status: ConsumptionStatus, reason: string | null = null): ConsumptionView {
  return {
    agent_id: "codex",
    asset: { domain: "model", profile_id: profile.id },
    desired: true,
    observed: status !== "drifted" || reason !== "model_target_missing",
    status,
    reason,
    affected_agent_ids: ["codex"],
  };
}

describe("describeAgentModel", () => {
  it("calls a central profile current only when observation is synced", () => {
    expect(describeAgentModel(profile, consumption("synced"), false)).toEqual({
      label: "Team OpenAI",
      detail: "gpt-test",
      synced: true,
    });
  });

  it("does not mislabel desired drift as the current model", () => {
    expect(
      describeAgentModel(profile, consumption("drifted", "model_owned_fields_drift"), false),
    ).toEqual({
      label: "配置已变更",
      detail: "期望：Team OpenAI · gpt-test",
      synced: false,
    });
    expect(
      describeAgentModel(profile, consumption("drifted", "model_target_missing"), false),
    ).toEqual({
      label: "配置缺失",
      detail: "期望：Team OpenAI · gpt-test",
      synced: false,
    });
  });

  it("distinguishes external and empty Agent configuration", () => {
    expect(describeAgentModel(null, null, true).label).toBe("外部配置");
    expect(describeAgentModel(null, null, false).label).toBe("未配置");
  });
});
