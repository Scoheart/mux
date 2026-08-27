# Clear Agent Models Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reviewed `清空 Models` action that replaces the selected Agent's managed Model selection with an empty set without deleting central assets.

**Architecture:** Reuse `AgentConsumptionPanel`'s existing bulk-remove control and Core's existing `set_agent_consumption` planner. Add one AgentView handler that always leaves the empty-selection plan pending for review instead of using the routine auto-commit path.

**Tech Stack:** React 19, TypeScript, Vitest, Testing Library, existing Rust asset planner and transaction engine.

---

### Task 1: Specify the bulk-clear interaction

**Files:**
- Modify: `desktop/src/components/AgentView.test.tsx`

- [ ] **Step 1: Add a regression test before production code**

Create a managed multi-model Agent fixture with two desired Models and one external observation. Render its Models tab, click `清空 Models`, and assert:

```ts
expect(planForAgent).toHaveBeenCalledWith("pi", {
  domain: "model",
  profile_ids: [],
});
expect(commit).not.toHaveBeenCalled();
```

Also assert the button is enabled with desired Models and that the external card remains read-only.

- [ ] **Step 2: Record the expected RED result**

The targeted command is:

```bash
cd desktop
npx vitest run src/components/AgentView.test.tsx
```

Before implementation it must fail because no button named `清空 Models` exists. Do not run this command under the repository's current fast-delivery policy unless the user explicitly authorizes tests.

### Task 2: Add the reviewed clear action

**Files:**
- Modify: `desktop/src/components/AgentView.tsx`

- [ ] **Step 1: Add a handler that always prepares review**

Add a dedicated handler beside `planRemoval`:

```ts
const clearModels = async () => {
  setPreparingChange(true);
  try {
    await consumptionState.planForAgent(agentId, {
      domain: "model",
      profile_ids: [],
    });
  } catch (error) {
    showToast({ kind: "error", msg: "无法准备清空 Models：" + formatError(error) });
  } finally {
    setPreparingChange(false);
  }
};
```

Do not call `commitPlan` here. The pending plan must flow through the existing `AssetOperationReviewDialog`.

- [ ] **Step 2: Wire the existing bulk control**

Pass these props to the Model `AgentConsumptionPanel`:

```tsx
bulkRemoveLabel="清空 Models"
bulkRemoveTitle={`移除 ${agent.name} 已添加的全部 Model`}
bulkRemoveDisabled={modelRows.length === 0
  || preparingChange
  || consumptionState.committing
  || changingModel !== null}
onBulkRemove={() => void clearModels()}
```

Keep `modelRows` as the enablement source so external observations cannot activate or enter the operation.

- [ ] **Step 3: Record the expected GREEN result**

The targeted command is:

```bash
cd desktop
npx vitest run src/components/AgentView.test.tsx
```

It should pass with the new test and all existing AgentView tests. Do not run it under the current repository policy without explicit user authorization.

### Task 3: Review and deliver the exact change

**Files:**
- Review: `desktop/src/components/AgentView.tsx`
- Review: `desktop/src/components/AgentView.test.tsx`
- Review: `docs/superpowers/specs/2026-08-27-clear-agent-models-design.md`
- Review: `docs/superpowers/plans/2026-08-27-clear-agent-models.md`

- [ ] **Step 1: Inspect the diff and scan the contract**

Confirm there is no change to central Model lifecycle code, Provider deletion, Keychain access, or version-owned files. Confirm the handler prepares exactly one empty selection and does not invoke `commit`.

- [ ] **Step 2: Create one remote feature commit**

Use the GitHub Git Data API with the exact four files above and commit message:

```text
feat(models): clear all Models from one Agent
```

Do not run local `git commit` or `git push`.

- [ ] **Step 3: Open, inspect, and merge the PR**

Create a PR from `codex/clear-agent-models` to `main`, verify its file list and remote blob identities, wait for required checks, then squash merge and delete the remote feature branch.

- [ ] **Step 4: Verify the resulting Stable release**

Track Direct Stable, verify the immutable tag and the four published assets, install and launch the verified Stable App, then review the installed Models panel at the requested Agent and viewport.
