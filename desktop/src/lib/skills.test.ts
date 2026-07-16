import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "./api";
import {
  filterSkills,
  installWizardReducer,
  resolveStagedResult,
} from "./skills";
import type {
  InventoryState,
  PlanAssignmentRequest,
  PlanImportRequest,
  PlanInstallRequest,
  PlanRemoveRequest,
  PlanRepairRequest,
  PlanUpdateRequest,
  SkillCommitRequest,
  SkillFileKind,
  SkillLocation,
  SkillSource,
} from "./types";
import {
  resolutionFixture,
  sharedTargetPlanFixture,
  skillsInventoryFixture,
} from "../test/skillsFixtures";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

const installRequest: PlanInstallRequest = {
  resolution_id: "resolve-fixture",
  skill_names: ["review-changes"],
  agent_ids: ["codex"],
  replace_conflicts: false,
};
const importRequest: PlanImportRequest = {
  identity: "target:cursor-user:legacy",
  agent_ids: ["cursor"],
  replace_conflicts: true,
};
const updateRequest: PlanUpdateRequest = {
  skill_name: "review-changes",
  replace_local_changes: false,
};
const removeRequest: PlanRemoveRequest = { skill_name: "review-changes" };
const assignmentRequest: PlanAssignmentRequest = {
  skill_name: "review-changes",
  agent_ids: ["codex"],
  enabled: true,
};
const repairRequest: PlanRepairRequest = {
  skill_name: "review-changes",
  repair: { kind: "target", target_id: "agents-user" },
};
const commitRequest: SkillCommitRequest = {
  operation_id: "resolve-fixture",
  candidate_hash: "candidate-hash",
  findings_confirmation: null,
};

beforeEach(() => {
  invokeMock.mockReset();
});

describe("Skills wire contracts", () => {
  it("keeps every discriminant and representative nested request in snake_case", () => {
    const sources: SkillSource[] = [
      {
        kind: "github",
        owner: "acme",
        repo: "skills",
        subpath: "catalog/review-changes",
        requested_ref: "main",
        pinned: false,
      },
      { kind: "local", path: "~/fixtures", subpath: "review-changes" },
      {
        kind: "imported",
        original_path: "~/.cursor/skills/review-changes",
        backup_path: "~/.mux/backups/skills/fixture/review-changes",
      },
    ];
    const locations: SkillLocation[] = [
      { kind: "central" },
      {
        kind: "agent_target",
        target_id: "agents-user",
        global_dir: "~/.agents/skills",
      },
    ];
    const states: InventoryState[] = [
      "managed",
      "assigned",
      "external",
      "locally_modified",
      "broken_link",
      "conflicting_link",
      "missing",
      "update_available",
    ];
    const fileKinds: SkillFileKind[] = ["file", "symlink"];

    expect(sources.map(({ kind }) => kind)).toEqual([
      "github",
      "local",
      "imported",
    ]);
    expect(locations.map(({ kind }) => kind)).toEqual([
      "central",
      "agent_target",
    ]);
    expect(states).toContain("locally_modified");
    expect(fileKinds).toEqual(["file", "symlink"]);
    expect(JSON.parse(JSON.stringify(repairRequest))).toEqual({
      skill_name: "review-changes",
      repair: { kind: "target", target_id: "agents-user" },
    });
  });

  it("invokes all 19 commands with exact top-level payload shapes", async () => {
    invokeMock.mockResolvedValue(undefined);
    const calls: Array<[
      () => Promise<unknown>,
      string,
      Record<string, unknown> | undefined,
    ]> = [
      [api.listSkillsInventory, "list_skills_inventory", undefined],
      [api.listSkillAgents, "list_skill_agents", undefined],
      [() => api.getSkillDetail("central:review-changes"), "get_skill_detail", { identity: "central:review-changes" }],
      [() => api.resolveGithubSkillSource("acme/skills"), "resolve_skill_source", { value: "acme/skills" }],
      [api.resolveLocalSkillSourceDialog, "resolve_local_skill_source_dialog", undefined],
      [() => api.planSkillInstall(installRequest), "plan_skill_install", { request: installRequest }],
      [() => api.commitSkillInstall(commitRequest), "commit_skill_install", { request: commitRequest }],
      [() => api.planSkillImport(importRequest), "plan_skill_import", { request: importRequest }],
      [() => api.commitSkillImport(commitRequest), "commit_skill_import", { request: commitRequest }],
      [() => api.planSkillUpdate(updateRequest), "plan_skill_update", { request: updateRequest }],
      [() => api.commitSkillUpdate(commitRequest), "commit_skill_update", { request: commitRequest }],
      [() => api.planSkillRemove(removeRequest), "plan_skill_remove", { request: removeRequest }],
      [() => api.commitSkillRemove(commitRequest), "commit_skill_remove", { request: commitRequest }],
      [() => api.planSkillAssignment(assignmentRequest), "plan_skill_assignment", { request: assignmentRequest }],
      [() => api.commitSkillAssignment(commitRequest), "commit_skill_assignment", { request: commitRequest }],
      [() => api.planSkillRepair(repairRequest), "plan_skill_repair", { request: repairRequest }],
      [() => api.commitSkillRepair(commitRequest), "commit_skill_repair", { request: commitRequest }],
      [() => api.checkSkillUpdates(true), "check_skill_updates", { manual: true }],
      [() => api.cancelSkillOperation("resolve-fixture"), "cancel_skill_operation", { operationId: "resolve-fixture" }],
    ];

    for (const [call, command, payload] of calls) {
      invokeMock.mockClear();
      await call();
      expect(invokeMock).toHaveBeenCalledTimes(1);
      if (payload) {
        expect(invokeMock).toHaveBeenCalledWith(command, payload);
      } else {
        expect(invokeMock).toHaveBeenCalledWith(command);
      }
    }
  });

  it("passes native picker cancellation through as null", async () => {
    invokeMock.mockResolvedValueOnce(null);
    await expect(api.resolveLocalSkillSourceDialog()).resolves.toBeNull();
  });
});

describe("filterSkills", () => {
  it("combines status, source, content kind, and search", () => {
    const result = filterSkills(skillsInventoryFixture().items, {
      status: "needs_attention",
      source: "github",
      contentKind: "automation",
      query: "REVIEW",
    });
    expect(result.map((item) => item.name)).toEqual(["review-changes"]);
  });

  it("groups imported backup snapshots under Local", () => {
    const imported = {
      ...skillsInventoryFixture().items[1],
      source: {
        kind: "imported" as const,
        original_path: "~/.cursor/skills/legacy",
        backup_path: "~/.mux/backups/skills/fixture/legacy",
      },
    };
    expect(
      filterSkills([imported], {
        status: "all",
        source: "local",
        contentKind: "all",
        query: "",
      }),
    ).toHaveLength(1);
  });

  it("treats high risk, updates, modification, link faults, and missing content as attention", () => {
    const base = skillsInventoryFixture().items[1];
    const attentionStates: InventoryState[] = [
      "locally_modified",
      "broken_link",
      "conflicting_link",
      "missing",
    ];
    const items = attentionStates.map((state, index) => ({
      ...base,
      identity: `central:attention-${index}`,
      name: `attention-${index}`,
      states: [state],
    }));
    expect(
      filterSkills(items, {
        status: "needs_attention",
        source: "all",
        contentKind: "all",
        query: "attention",
      }),
    ).toHaveLength(attentionStates.length);
  });

  it("matches ASCII I without depending on the process locale", () => {
    vi.spyOn(String.prototype, "toLocaleLowerCase").mockImplementation(
      function localeLower(this: string) {
        return String(this).replace(/I/g, "ı").toLowerCase();
      },
    );
    const item = {
      ...skillsInventoryFixture().items[1],
      name: "INSTALL-HELPER",
      description: "Install a local Skill",
    };

    expect(
      filterSkills([item], {
        status: "all",
        source: "all",
        contentKind: "all",
        query: "install",
      }),
    ).toEqual([item]);
  });
});

describe("installWizardReducer", () => {
  it("selects all candidates but no Agents when a resolution loads", () => {
    const state = installWizardReducer(undefined, {
      type: "resolution_loaded",
      resolution: resolutionFixture(),
    });
    expect(state.selectedSkillNames).toEqual(["review-changes"]);
    expect(state.selectedAgentIds).toEqual([]);
    expect(state.plan).toBeNull();
  });

  it("invalidates a loaded plan whenever Skill or Agent selection changes", () => {
    let state = installWizardReducer(undefined, {
      type: "resolution_loaded",
      resolution: resolutionFixture(),
    });
    state = installWizardReducer(state, {
      type: "plan_loaded",
      plan: sharedTargetPlanFixture(),
    });
    state = installWizardReducer(state, {
      type: "toggle_agent",
      agentId: "codex",
    });
    expect(state.plan).toBeNull();
    state = installWizardReducer(state, {
      type: "plan_loaded",
      plan: sharedTargetPlanFixture(),
    });
    state = installWizardReducer(state, {
      type: "toggle_skill",
      skillName: "review-changes",
    });
    expect(state.plan).toBeNull();
  });
});

describe("resolveStagedResult", () => {
  it("best-effort cancels and discards a late staged operation", async () => {
    const cancel = vi.fn().mockRejectedValue(new Error("cleanup failed"));
    await expect(
      resolveStagedResult(
        Promise.resolve(resolutionFixture()),
        () => false,
        cancel,
      ),
    ).resolves.toBeNull();
    expect(cancel).toHaveBeenCalledWith("resolve-fixture");
  });

  it("best-effort cancels a discarded plan by its exact shared operation id", async () => {
    const cancel = vi.fn().mockResolvedValue(undefined);
    await expect(
      resolveStagedResult(
        Promise.resolve(sharedTargetPlanFixture()),
        () => false,
        cancel,
      ),
    ).resolves.toBeNull();
    expect(cancel).toHaveBeenCalledWith("resolve-fixture");
  });

  it("leaves picker cancellation unchanged and creates no cancellation", async () => {
    const cancel = vi.fn();
    await expect(
      resolveStagedResult(Promise.resolve(null), () => false, cancel),
    ).resolves.toBeNull();
    expect(cancel).not.toHaveBeenCalled();
  });
});
