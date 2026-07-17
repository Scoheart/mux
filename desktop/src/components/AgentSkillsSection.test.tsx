import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { InstallState } from "../hooks/useInstallState";
import type { SkillsState } from "../hooks/useSkillsState";
import * as api from "../lib/api";
import type {
  OperationPlan,
  SkillInventoryItem,
  SkillsInventory,
} from "../lib/types";
import {
  sharedTargetPlanFixture,
  skillsInventoryFixture,
  skillsStateFixture,
} from "../test/skillsFixtures";
import { AgentSkillsSection } from "./AgentSkillsSection";
import { AgentView } from "./AgentView";
import { ToastProvider } from "./Toast";

const agentCss = await readFile(resolve(process.cwd(), "src/index.css"), "utf8");
const agentSkillsSource = await readFile(
  resolve(process.cwd(), "src/components/AgentSkillsSection.tsx"),
  "utf8",
);

function groupedDeclarations(source: string, selector: string): string | null {
  const uncommented = source.replace(/\/\*[\s\S]*?\*\//g, "");
  for (const match of uncommented.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const selectors = match[1].split(",").map((candidate) => candidate.trim());
    if (selectors.includes(selector)) return match[2];
  }
  return null;
}

function mediaBlock(source: string, heading: string): string {
  const start = source.indexOf(heading);
  const openingBrace = source.indexOf("{", start + heading.length);
  let depth = 0;
  for (let index = openingBrace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") depth -= 1;
    if (depth === 0) return source.slice(start, index + 1);
  }
  return "";
}

vi.mock("../lib/api", async () => {
  const actual = await vi.importActual<typeof import("../lib/api")>("../lib/api");
  return {
    ...actual,
    planSkillAssignment: vi.fn(),
    listModelProfiles: vi.fn(),
    listModelAgents: vi.fn(),
  };
});

afterEach(cleanup);

interface Deferred<T> {
  promise: Promise<T>;
  resolve(value: T): void;
  reject(reason: unknown): void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, decline) => {
    resolve = accept;
    reject = decline;
  });
  return { promise, resolve, reject };
}

function assignmentPlan(operationId = "assignment-operation"): OperationPlan {
  return {
    ...sharedTargetPlanFixture(),
    operation_id: operationId,
    kind: "assignment",
  };
}

function targetItem(
  name: string,
  targetId: string,
  states: SkillInventoryItem["states"] = ["assigned"],
): SkillInventoryItem {
  const base = skillsInventoryFixture().items[0];
  return {
    ...base,
    identity: `target:${targetId}:${name}`,
    name,
    states,
    location: {
      kind: "agent_target",
      target_id: targetId,
      global_dir: "~/.wrong-item-path/skills",
    },
    source: null,
    assigned_target_ids: [],
  };
}

function graphInventory(): SkillsInventory {
  const inventory = skillsInventoryFixture();
  const central = {
    ...inventory.items[0],
    states: ["managed", "update_available"] as SkillInventoryItem["states"],
    assigned_target_ids: ["agents-user", "cursor-only"],
  };
  const otherCentral = {
    ...inventory.items[1],
    assigned_target_ids: ["cursor-only"],
  };
  inventory.items = [
    central,
    otherCentral,
    targetItem("review-changes", "agents-user"),
    targetItem("external-copy", "agents-user", ["external"]),
  ];
  inventory.targets = [
    {
      target_id: "agents-user",
      global_dir: "~/.shared/actual-skills",
      primary_agent_ids: ["codex"],
      affected_agent_ids: ["codex", "cursor", "gemini"],
      assignable: true,
    },
    {
      target_id: "cursor-only",
      global_dir: "~/.cursor/private-skills",
      primary_agent_ids: ["cursor"],
      affected_agent_ids: ["cursor", "ghost-agent"],
      assignable: true,
    },
  ];
  return inventory;
}

function twoAssignmentInventory(): SkillsInventory {
  const inventory = graphInventory();
  inventory.items[1] = {
    ...inventory.items[1],
    assigned_target_ids: ["agents-user"],
  };
  inventory.items.push(targetItem("unassigned-skill", "agents-user"));
  return inventory;
}

function stateWith(
  inventory: SkillsInventory | null,
  overrides: Partial<SkillsState> = {},
): SkillsState {
  return { ...skillsStateFixture(), inventory, ...overrides };
}

function renderSection(
  inventory: SkillsInventory | null,
  overrides: Partial<SkillsState> = {},
) {
  const onOpenSkills = vi.fn();
  const rendered = render(
    <AgentSkillsSection
      agentId="codex"
      state={stateWith(inventory, overrides)}
      onOpenSkills={onOpenSkills}
    />,
  );
  return { ...rendered, onOpenSkills };
}

function installState(hasGlobal = true): InstallState {
  const agent = {
    id: "codex",
    name: "Codex",
    format: "toml",
    key: "mcp_servers",
    has_global: hasGlobal,
    has_project: false,
    enabled: true,
    supported_transports: ["stdio", "http"] as Array<"stdio" | "http">,
    global: hasGlobal ? "~/.codex/config.toml" : null,
    project: null,
    skills_global_dir: hasGlobal ? "~/.agents/skills" : null,
    docs: "https://example.invalid/codex",
    note: hasGlobal ? null : "仅作参考，不提供可写配置。",
    category: "coding",
    evidence: "official" as const,
    verified_at: "2026-07-17",
    builtin: true,
  };
  return {
    entries: [],
    catalog: [],
    agents: [agent],
    installed: [],
    loading: false,
    pending: new Set(),
    agentsForServer: () => [],
    customKeys: new Set(),
    toggle: vi.fn(async () => undefined),
    setEnabled: vi.fn(async () => undefined),
    remove: vi.fn(async () => undefined),
    rescan: vi.fn(async () => []),
    refreshAll: vi.fn(async () => undefined),
    refreshRegistry: vi.fn(async () => []),
    refreshAgents: vi.fn(async () => [agent]),
    sources: [],
    refreshSources: vi.fn(async () => []),
    subscribe: vi.fn(),
    pickLocalSource: vi.fn(async () => null),
    rescanDiscovered: vi.fn(async () => undefined),
    refreshOneSource: vi.fn(async () => undefined),
    toggleSource: vi.fn(async () => undefined),
    deleteSource: vi.fn(async () => undefined),
    importPaste: vi.fn(async () => []),
  };
}

beforeEach(() => {
  vi.mocked(api.planSkillAssignment).mockResolvedValue(assignmentPlan());
  vi.mocked(api.listModelProfiles).mockResolvedValue([]);
  vi.mocked(api.listModelAgents).mockResolvedValue([]);
});

describe("AgentSkillsSection target graph", () => {
  it("derives assignments from affected targets and shows only their actual path and impact", () => {
    renderSection(graphInventory());

    const region = screen.getByRole("region", { name: "Skills" });
    expect(within(region).getAllByRole("heading", { name: "Skills" })).toHaveLength(1);
    expect(within(region).getByRole("heading", { name: "Skills" })).toHaveAttribute(
      "id",
      "agent-skills-title",
    );
    const row = within(region).getByRole("listitem", {
      name: /review-changes/,
    });
    expect(within(row).getByText("~/.shared/actual-skills")).toBeVisible();
    expect(within(row).getByText(/Codex、Cursor、Gemini CLI/)).toBeVisible();
    expect(within(row).getByText(/当前生效/)).toBeVisible();
    expect(within(row).getByRole("switch", { name: "停用 review-changes" })).toHaveAttribute(
      "aria-checked",
      "true",
    );

    expect(screen.queryByText("unassigned-skill")).not.toBeInTheDocument();
    expect(screen.queryByText("external-copy")).not.toBeInTheDocument();
    expect(screen.queryByText("~/.cursor/private-skills")).not.toBeInTheDocument();
    expect(screen.queryByText("ghost-agent")).not.toBeInTheDocument();
    expect(screen.queryByText("~/.wrong-item-path/skills")).not.toBeInTheDocument();
  });

  it.each([
    {
      label: "missing target",
      actualStates: null,
      centralStates: ["managed"] as SkillInventoryItem["states"],
      status: "当前未生效（目标缺失）",
      canDisable: true,
    },
    {
      label: "broken link",
      actualStates: ["broken_link"] as SkillInventoryItem["states"],
      centralStates: ["managed"] as SkillInventoryItem["states"],
      status: "当前未生效（链接损坏）",
      canDisable: false,
    },
    {
      label: "conflicting link",
      actualStates: ["conflicting_link"] as SkillInventoryItem["states"],
      centralStates: ["managed"] as SkillInventoryItem["states"],
      status: "当前未生效（链接冲突）",
      canDisable: false,
    },
    {
      label: "broken central state",
      actualStates: ["assigned"] as SkillInventoryItem["states"],
      centralStates: ["managed", "broken_link"] as SkillInventoryItem["states"],
      status: "当前未生效（链接损坏）",
      canDisable: false,
    },
    {
      label: "conflicting central state",
      actualStates: ["assigned"] as SkillInventoryItem["states"],
      centralStates: ["managed", "conflicting_link"] as SkillInventoryItem["states"],
      status: "当前未生效（链接冲突）",
      canDisable: false,
    },
    {
      label: "locally modified central copy",
      actualStates: ["assigned"] as SkillInventoryItem["states"],
      centralStates: ["managed", "locally_modified"] as SkillInventoryItem["states"],
      status: "本地已修改，需在 Skills 中审阅",
      canDisable: false,
    },
  ])("keeps desired assignment distinct from $label reality", ({
    actualStates,
    centralStates,
    status,
    canDisable,
  }) => {
    const inventory = graphInventory();
    inventory.items[0] = { ...inventory.items[0], states: centralStates };
    inventory.items = inventory.items.filter(
      (item) => item.identity !== "target:agents-user:review-changes",
    );
    if (actualStates) {
      inventory.items.push(targetItem("review-changes", "agents-user", actualStates));
    }

    renderSection(inventory);

    const row = screen.getByRole("listitem", { name: /review-changes/ });
    expect(within(row).getByText("已分配", { exact: false })).toBeVisible();
    expect(within(row).getByText(status, { exact: false })).toBeVisible();
    if (canDisable) {
      expect(within(row).getByRole("switch", { name: "停用 review-changes" })).toBeEnabled();
    } else {
      expect(within(row).queryByRole("switch")).not.toBeInTheDocument();
      expect(within(row).getByRole("button", { name: "查看 review-changes 详情" })).toBeVisible();
    }
  });

  it("keeps a details action separate from the assignment switch", async () => {
    const user = userEvent.setup();
    const { onOpenSkills } = renderSection(graphInventory());
    const row = screen.getByRole("listitem", { name: /review-changes/ });

    expect(within(row).getByRole("switch", { name: "停用 review-changes" })).toBeVisible();
    await user.click(within(row).getByRole("button", { name: "查看 review-changes 详情" }));

    expect(onOpenSkills).toHaveBeenCalledWith({
      kind: "detail",
      skillName: "review-changes",
    });
    expect(api.planSkillAssignment).not.toHaveBeenCalled();
  });

  it("shows the central Skill's short risk and update evidence", () => {
    renderSection(graphInventory());
    const row = screen.getByRole("listitem", { name: /review-changes/ });

    expect(within(row).getByText("高风险")).toBeVisible();
    expect(within(row).getByText("有更新")).toBeVisible();
  });

  it("keeps an assigned central row with no source as read-only needs-attention evidence", async () => {
    const user = userEvent.setup();
    const inventory = graphInventory();
    inventory.items[0] = { ...inventory.items[0], source: null };
    const { onOpenSkills } = renderSection(inventory);
    const row = screen.getByRole("listitem", { name: /review-changes/ });

    expect(within(row).getByText(/来源异常，需处理/)).toBeVisible();
    expect(within(row).queryByRole("switch")).not.toBeInTheDocument();
    await user.click(within(row).getByRole("button", { name: "查看 review-changes 详情" }));
    expect(onOpenSkills).toHaveBeenCalledWith({
      kind: "detail",
      skillName: "review-changes",
    });
  });
});

describe("AgentSkillsSection inventory states", () => {
  it("shows inline loading and retries an initial inventory error", async () => {
    const user = userEvent.setup();
    const { rerender } = renderSection(null, { loading: true, error: null });
    expect(screen.getByText("正在读取 Skills…")).toBeVisible();

    const refresh = vi.fn().mockResolvedValue(graphInventory());
    rerender(
      <AgentSkillsSection
        agentId="codex"
        state={stateWith(null, {
          loading: false,
          error: { code: "read_failed", message: "inventory unavailable" },
          refresh,
        })}
        onOpenSkills={vi.fn()}
      />,
    );
    expect(screen.getByText("读取 Skills 失败")).toBeVisible();
    expect(screen.getByText("inventory unavailable")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "重试" }));
    expect(refresh).toHaveBeenCalledOnce();
  });

  it("treats an initial recovery error as read-only instead of a retryable read failure", () => {
    renderSection(null, {
      loading: false,
      error: {
        code: "recovery_required",
        message: "recover the interrupted journal",
      },
    });

    expect(screen.getByRole("status")).toHaveTextContent("recover the interrupted journal");
    expect(screen.queryByText("读取 Skills 失败")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "重试" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "添加 Skill" })).not.toBeInTheDocument();
  });

  it("keeps cached rows visible under a non-destructive error banner", () => {
    renderSection(graphInventory(), {
      error: { code: "rate_limited", message: "更新检查暂时失败" },
    });

    expect(screen.getByRole("status")).toHaveTextContent("更新检查暂时失败");
    expect(screen.getByRole("listitem", { name: /review-changes/ })).toBeVisible();
  });

  it("keeps recovery rows navigable while disabling Add and every switch", async () => {
    const user = userEvent.setup();
    const inventory = graphInventory();
    inventory.recovery_error = "journal recovery required";
    const { onOpenSkills } = renderSection(inventory);

    expect(screen.getByRole("status")).toHaveTextContent("journal recovery required");
    expect(screen.getByRole("button", { name: "添加 Skill" })).toBeDisabled();
    expect(screen.getByRole("switch", { name: "停用 review-changes" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "查看 review-changes 详情" }));
    expect(onOpenSkills).toHaveBeenCalledWith({
      kind: "detail",
      skillName: "review-changes",
    });
  });

  it("shows a true verified empty state whose Add action targets exactly this Agent", async () => {
    const user = userEvent.setup();
    const inventory = graphInventory();
    inventory.items = [];
    const { onOpenSkills } = renderSection(inventory);

    expect(screen.getByText("还没有分配 Skill")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "添加 Skill" }));
    expect(onOpenSkills).toHaveBeenCalledWith({ kind: "install", agentId: "codex" });
  });

  it("shows capability unavailable when the Agent is not in verified Skills inventory", () => {
    const inventory = graphInventory();
    inventory.agents = inventory.agents.filter((agent) => agent.id !== "codex");
    renderSection(inventory);

    expect(screen.getByText("此 Agent 暂不支持 Skills")).toBeVisible();
    expect(screen.queryByRole("button", { name: "添加 Skill" })).not.toBeInTheDocument();
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
  });

  it("disables every assignment switch while another Skills commit is pending", () => {
    renderSection(graphInventory(), { pendingOperation: "another-operation" });
    expect(screen.getByRole("switch", { name: "停用 review-changes" })).toBeDisabled();
  });
});

describe("AgentSkillsSection assignment review", () => {
  it("imports only the assignment planner and receives commit through state", () => {
    expect(agentSkillsSource).not.toMatch(/import\s+\*\s+as\s+api/);
    expect(agentSkillsSource).not.toMatch(/commitSkill(?:Install|Import|Update|Remove|Assignment|Repair)/);
    expect(agentSkillsSource).toMatch(/import\s+\{\s*planSkillAssignment\s*\}/);
  });

  it("plans the exact disable request once without optimistically flipping the switch", async () => {
    const pending = deferred<OperationPlan>();
    vi.mocked(api.planSkillAssignment).mockReturnValueOnce(pending.promise);
    renderSection(graphInventory());
    const assignmentSwitch = screen.getByRole("switch", {
      name: "停用 review-changes",
    });

    fireEvent.click(assignmentSwitch);
    fireEvent.click(assignmentSwitch);

    expect(api.planSkillAssignment).toHaveBeenCalledOnce();
    expect(api.planSkillAssignment).toHaveBeenCalledWith({
      skill_name: "review-changes",
      agent_ids: ["codex"],
      enabled: false,
    });
    expect(assignmentSwitch).toHaveAttribute("aria-checked", "true");
    expect(assignmentSwitch).toBeDisabled();
    expect(screen.getByRole("status")).toHaveTextContent(
      "正在生成 review-changes 分配计划…",
    );
    expect(screen.getByRole("listitem", { name: /review-changes/ })).toHaveAttribute(
      "aria-busy",
      "true",
    );

    await act(async () => pending.resolve(assignmentPlan()));
    expect(await screen.findByRole("dialog", { name: "审阅 Skill 操作" })).toBeVisible();
    expect(screen.getByRole("switch", { name: "停用 review-changes" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  it("keeps every row disabled after a plan resolves and while review is open", async () => {
    const user = userEvent.setup();
    renderSection(twoAssignmentInventory());

    await user.click(screen.getByRole("switch", { name: "停用 review-changes" }));
    await screen.findByRole("dialog", { name: "审阅 Skill 操作" });

    expect(screen.getByRole("switch", { name: "停用 review-changes" })).toBeDisabled();
    expect(screen.getByRole("switch", { name: "停用 unassigned-skill" })).toBeDisabled();
    await user.click(screen.getByRole("switch", { name: "停用 unassigned-skill" }));
    expect(api.planSkillAssignment).toHaveBeenCalledOnce();
  });

  it("shows a plan failure and re-enables assignment controls", async () => {
    const user = userEvent.setup();
    vi.mocked(api.planSkillAssignment).mockRejectedValueOnce({
      code: "plan_failed",
      message: "目标状态刚刚变化",
    });
    renderSection(graphInventory());

    await user.click(screen.getByRole("switch", { name: "停用 review-changes" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("目标状态刚刚变化");
    expect(screen.getByRole("switch", { name: "停用 review-changes" })).toBeEnabled();
  });

  it("enters a local read-only recovery state when planning requires recovery", async () => {
    const user = userEvent.setup();
    vi.mocked(api.planSkillAssignment).mockRejectedValueOnce({
      code: "recovery_required",
      message: "recover assignment journal",
    });
    renderSection(graphInventory());

    await user.click(screen.getByRole("switch", { name: "停用 review-changes" }));

    expect(await screen.findByRole("status")).toHaveTextContent("recover assignment journal");
    expect(screen.getByRole("button", { name: "添加 Skill" })).toBeDisabled();
    expect(screen.getByRole("switch", { name: "停用 review-changes" })).toBeDisabled();
  });

  it("reviews shared loss context and commits only through the injected state", async () => {
    const user = userEvent.setup();
    const inventory = graphInventory();
    const afterCommit = { ...inventory, items: [] };
    const plan = assignmentPlan("assignment-reviewed");
    plan.targets.push({
      target_id: "cursor-only",
      global_dir: "~/.cursor/private-skills",
      expected: "managed",
      primary_agent_ids: ["cursor"],
      affected_agent_ids: ["cursor", "ghost-agent"],
    });
    const commit = vi.fn().mockResolvedValue(afterCommit);
    const cancel = vi.fn().mockResolvedValue(undefined);
    vi.mocked(api.planSkillAssignment).mockResolvedValueOnce(plan);
    const rendered = renderSection(inventory, { commit, cancel });

    await user.click(screen.getByRole("switch", { name: "停用 review-changes" }));
    const review = await screen.findByRole("dialog", { name: "审阅 Skill 操作" });
    const impact = within(review).getByRole("region", { name: "分配影响" });
    expect(within(impact).getByText("将停止为 Codex 分配")).toBeVisible();
    expect(within(impact).getByText(/Codex、Cursor、Gemini CLI 将失去访问/)).toBeVisible();
    expect(within(impact).queryByText("~/.cursor/private-skills")).not.toBeInTheDocument();
    expect(within(impact).queryByText(/ghost-agent.*将失去访问/)).not.toBeInTheDocument();
    expect(commit).not.toHaveBeenCalled();

    await user.click(within(review).getByRole("button", { name: "确认更改分配" }));
    await waitFor(() => expect(commit).toHaveBeenCalledWith(plan, null));
    expect(screen.queryByRole("dialog", { name: "审阅 Skill 操作" })).not.toBeInTheDocument();
    expect(cancel).not.toHaveBeenCalled();

    rendered.rerender(
      <AgentSkillsSection
        agentId="codex"
        state={stateWith(afterCommit, { commit, cancel })}
        onOpenSkills={rendered.onOpenSkills}
      />,
    );
    expect(screen.queryByRole("listitem", { name: /review-changes/ })).not.toBeInTheDocument();
    expect(screen.getByText("还没有分配 Skill")).toBeVisible();
  });

  it("cancels a closed review once and restores focus to its switch", async () => {
    const user = userEvent.setup();
    const cancel = vi.fn().mockResolvedValue(undefined);
    renderSection(graphInventory(), { cancel });
    const assignmentSwitch = screen.getByRole("switch", {
      name: "停用 review-changes",
    });

    await user.click(assignmentSwitch);
    await screen.findByRole("dialog", { name: "审阅 Skill 操作" });
    await user.keyboard("{Escape}");

    await waitFor(() => expect(cancel).toHaveBeenCalledWith("assignment-operation"));
    expect(cancel).toHaveBeenCalledOnce();
    expect(screen.queryByRole("dialog", { name: "审阅 Skill 操作" })).not.toBeInTheDocument();
    await waitFor(() => expect(assignmentSwitch).toHaveFocus());
  });

  it("cancels an open review once when the section unmounts", async () => {
    const user = userEvent.setup();
    const cancel = vi.fn().mockResolvedValue(undefined);
    const rendered = renderSection(graphInventory(), { cancel });

    await user.click(screen.getByRole("switch", { name: "停用 review-changes" }));
    await screen.findByRole("dialog", { name: "审阅 Skill 操作" });
    rendered.unmount();

    await waitFor(() => expect(cancel).toHaveBeenCalledWith("assignment-operation"));
    expect(cancel).toHaveBeenCalledOnce();
  });

  it("generation-guards and cancels a late plan after the Agent changes", async () => {
    const user = userEvent.setup();
    const pending = deferred<OperationPlan>();
    const cancel = vi.fn().mockResolvedValue(undefined);
    vi.mocked(api.planSkillAssignment).mockReturnValueOnce(pending.promise);
    const rendered = renderSection(graphInventory(), { cancel });

    await user.click(screen.getByRole("switch", { name: "停用 review-changes" }));
    rendered.rerender(
      <AgentSkillsSection
        agentId="cursor"
        state={stateWith(graphInventory(), { cancel })}
        onOpenSkills={rendered.onOpenSkills}
      />,
    );
    await act(async () => pending.resolve(assignmentPlan("late-agent-plan")));

    await waitFor(() => expect(cancel).toHaveBeenCalledWith("late-agent-plan"));
    expect(screen.queryByRole("dialog", { name: "审阅 Skill 操作" })).not.toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "停用 review-changes" })).toBeEnabled();
  });

  it("cancels a plan that resolves after unmount", async () => {
    const user = userEvent.setup();
    const pending = deferred<OperationPlan>();
    const cancel = vi.fn().mockResolvedValue(undefined);
    vi.mocked(api.planSkillAssignment).mockReturnValueOnce(pending.promise);
    const rendered = renderSection(graphInventory(), { cancel });

    await user.click(screen.getByRole("switch", { name: "停用 review-changes" }));
    rendered.unmount();
    await act(async () => pending.resolve(assignmentPlan("late-unmount-plan")));

    await waitFor(() => expect(cancel).toHaveBeenCalledWith("late-unmount-plan"));
    expect(cancel).toHaveBeenCalledOnce();
  });

  it("waits for an in-flight successful commit on unmount without cancelling it", async () => {
    const user = userEvent.setup();
    const committing = deferred<SkillsInventory>();
    const cancel = vi.fn().mockResolvedValue(undefined);
    const commit = vi.fn(() => committing.promise);
    const inventory = graphInventory();
    const rendered = renderSection(inventory, { cancel, commit });

    await user.click(screen.getByRole("switch", { name: "停用 review-changes" }));
    await user.click(await screen.findByRole("button", { name: "确认更改分配" }));
    rendered.unmount();
    expect(cancel).not.toHaveBeenCalled();

    committing.resolve({ ...inventory, items: [] });
    await act(async () => committing.promise);
    expect(cancel).not.toHaveBeenCalled();
  });
});

describe("AgentView Skills placement", () => {
  it("shows independent Model, MCP, and Skills configuration locations", async () => {
    vi.mocked(api.listModelAgents).mockResolvedValueOnce([{
      id: "codex",
      name: "Codex",
      mode: "managed",
      installed: true,
      config_path: "~/.codex/config.toml",
      docs: "https://example.invalid/models",
      assigned_profile: null,
      supported_protocols: ["openai-responses"],
      note: "",
    }]);
    const inventory = graphInventory();
    inventory.agents = [];

    render(
      <ToastProvider>
        <AgentView
          state={installState()}
          skillsState={stateWith(inventory)}
          agentId="codex"
          onOpenModels={vi.fn()}
          onOpenSkills={vi.fn()}
        />
      </ToastProvider>,
    );

    expect(await screen.findByText("Model / MCP 共用")).toBeVisible();
    const region = screen.getByRole("region", { name: "配置位置" });
    expect(within(region).getByText("Model")).toBeVisible();
    expect(within(region).getByText("MCP")).toBeVisible();
    expect(within(region).getByText("Skills")).toBeVisible();
    expect(within(region).getAllByText("~/.codex/config.toml")).toHaveLength(2);
    expect(within(region).getByText("~/.agents/skills")).toBeVisible();
    expect(within(region).getByText("已核验目录 · 未检测到 Agent")).toBeVisible();
  });

  it("does not copy the MCP path into unavailable capabilities", async () => {
    const state = installState();
    state.agents[0] = { ...state.agents[0], skills_global_dir: null };
    render(
      <ToastProvider>
        <AgentView
          state={state}
          skillsState={stateWith(graphInventory())}
          agentId="codex"
          onOpenModels={vi.fn()}
          onOpenSkills={vi.fn()}
        />
      </ToastProvider>,
    );

    const region = screen.getByRole("region", { name: "配置位置" });
    expect(await within(region).findByText("尚未接入 Models")).toBeVisible();
    expect(within(region).getByText("尚未核验 Skills 目录")).toBeVisible();
    expect(within(region).getAllByText("~/.codex/config.toml")).toHaveLength(1);
  });

  it("places Skills between Model and MCP on writable Agent pages", async () => {
    const rendered = render(
      <ToastProvider>
        <AgentView
          state={installState()}
          skillsState={stateWith(graphInventory())}
          agentId="codex"
          onOpenModels={vi.fn()}
          onOpenSkills={vi.fn()}
        />
      </ToastProvider>,
    );

    await waitFor(() => expect(api.listModelAgents).toHaveBeenCalled());
    const headings = within(rendered.container)
      .getAllByRole("heading")
      .map((heading) => heading.textContent);
    expect(headings.indexOf("Model")).toBeLessThan(headings.indexOf("Skills"));
    expect(headings.indexOf("Skills")).toBeLessThan(headings.indexOf("MCP"));
  });

  it("leaves reference-only Agent pages unchanged", async () => {
    render(
      <ToastProvider>
        <AgentView
          state={installState(false)}
          skillsState={stateWith(graphInventory())}
          agentId="codex"
          onOpenModels={vi.fn()}
          onOpenSkills={vi.fn()}
        />
      </ToastProvider>,
    );

    expect(screen.getByText("仅作参考，不提供可写配置。")).toBeVisible();
    expect(screen.queryByRole("heading", { name: "Skills" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "MCP" })).not.toBeInTheDocument();
  });

  it("keeps Skill evidence readable and actions reachable at 900×600", () => {
    expect(groupedDeclarations(agentCss, ".mux-agent-file-map")).toMatch(
      /grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\)/,
    );
    expect(groupedDeclarations(agentCss, ".mux-agent-file-copy code")).toMatch(
      /overflow-wrap:\s*anywhere/,
    );
    expect(groupedDeclarations(agentCss, ".mux-agent-page")).toMatch(/overflow-y:\s*auto/);
    expect(groupedDeclarations(agentCss, ".mux-agent-skill-row")).toMatch(
      /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+auto/,
    );
    expect(groupedDeclarations(agentCss, ".mux-agent-skill-path")).toMatch(
      /overflow-wrap:\s*anywhere/,
    );
    expect(groupedDeclarations(agentCss, ".mux-agent-skill-actions")).toMatch(
      /flex:\s*0\s+0\s+auto/,
    );
    expect(
      groupedDeclarations(agentCss, '.mux-agent-skill-actions [role="switch"]:focus-visible'),
    ).toMatch(/outline:/);

    const narrowHeading = "@media (max-width: 920px)";
    const narrow = mediaBlock(
      agentCss.slice(agentCss.lastIndexOf(narrowHeading)),
      narrowHeading,
    );
    expect(narrow).toContain(".mux-agent-skill-row");
    expect(narrow).toMatch(/grid-template-columns:\s*minmax\(0,\s*1fr\)/);
    const reducedHeading = "@media (prefers-reduced-motion: reduce)";
    const reduced = mediaBlock(
      agentCss.slice(agentCss.lastIndexOf(reducedHeading)),
      reducedHeading,
    );
    expect(reduced).toContain('.mux-agent-skill-actions [role="switch"] > span');
    expect(reduced).toMatch(/transition:\s*none/);
  });
});
