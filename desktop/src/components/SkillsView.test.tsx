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
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../lib/api";
import type { SkillsState } from "../hooks/useSkillsState";
import type {
  SkillDetail,
  SkillInventoryItem,
  SkillsInventory,
  UpdateCheckOutcome,
} from "../lib/types";
import {
  resolutionFixture,
  sharedTargetPlanFixture,
  skillDetailFixture,
  skillsInventoryFixture,
  skillsStateFixture,
} from "../test/skillsFixtures";
import App from "../App";
import { SkillsView } from "./SkillsView";

const appMocks = vi.hoisted(() => ({
  useInstallState: vi.fn(),
  useSkillsState: vi.fn(),
  useUpdater: vi.fn(),
  useCliTool: vi.fn(),
  usePinnedAgents: vi.fn(),
  getVersion: vi.fn(),
  agentViewProps: vi.fn(),
}));

vi.mock("../lib/api", async () => {
  const actual = await vi.importActual<typeof import("../lib/api")>("../lib/api");
  return {
    ...actual,
    getSkillDetail: vi.fn(),
    planSkillAssignment: vi.fn(),
    resolveGithubSkillSource: vi.fn(),
  };
});
vi.mock("../hooks/useInstallState", () => ({
  useInstallState: appMocks.useInstallState,
}));
vi.mock("../hooks/useSkillsState", async () => {
  const actual = await vi.importActual<typeof import("../hooks/useSkillsState")>(
    "../hooks/useSkillsState",
  );
  return { ...actual, useSkillsState: appMocks.useSkillsState };
});
vi.mock("../hooks/useUpdater", () => ({ useUpdater: appMocks.useUpdater }));
vi.mock("../hooks/useCliTool", () => ({ useCliTool: appMocks.useCliTool }));
vi.mock("../hooks/usePinnedAgents", () => ({
  usePinnedAgents: appMocks.usePinnedAgents,
}));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: appMocks.getVersion }));
vi.mock("./RegistryView", () => ({
  RegistryView: () => <div>registry-view</div>,
}));
vi.mock("./ModelsView", () => ({ ModelsView: () => <div>models-view</div> }));
vi.mock("./AgentView", () => ({
  AgentView: (props: {
    agentId: string;
    skillsState?: SkillsState;
    onOpenSkills?: (request: {
      kind: "detail";
      skillName: string;
    } | {
      kind: "install";
      agentId: string;
    }) => void;
  }) => {
    appMocks.agentViewProps(props);
    return (
      <div>
        agent-view:{props.agentId}
        <button
          type="button"
          onClick={() =>
            props.onOpenSkills?.({ kind: "detail", skillName: "review-changes" })
          }
        >
          查看 Agent Skill
        </button>
        <button
          type="button"
          onClick={() =>
            props.onOpenSkills?.({ kind: "install", agentId: props.agentId })
          }
        >
          为 Agent 添加 Skill
        </button>
      </div>
    );
  },
}));
vi.mock("./AddAgentDialog", () => ({ AddAgentDialog: () => null }));
vi.mock("./RegistryEditPage", () => ({ RegistryEditPage: () => null }));
vi.mock("./UpdateBanner", () => ({ UpdateBanner: () => null }));

const stateWith = (
  inventory: SkillsInventory | null,
  overrides: Partial<SkillsState> = {},
): SkillsState => ({
  ...skillsStateFixture(),
  inventory,
  ...overrides,
});

const importedItem = (): SkillInventoryItem => ({
  ...skillsInventoryFixture().items[1],
  identity: "central:imported-legacy",
  name: "imported-legacy",
  description: "Imported local instructions",
  content_kind: "instructions",
  source: {
    kind: "imported",
    original_path: "~/.cursor/skills/imported-legacy",
    backup_path: "~/.mux/backups/skills/fixture/imported-legacy",
  },
});

const appAgent = (index: number) => ({
  id: `agent-${index}`,
  name: `Agent ${index}`,
  format: "json",
  key: "mcpServers",
  has_global: true,
  has_project: false,
  enabled: true,
  supported_transports: ["stdio" as const, "http" as const],
  global: `~/.agent-${index}/settings.json`,
  project: null,
  skills_global_dir: null,
  docs: null,
  note: null,
  category: "coding",
  evidence: "official",
  verified_at: null,
  builtin: true,
});

const installStateForApp = (loading: boolean) => ({
  agents: Array.from({ length: 6 }, (_, index) => appAgent(index + 1)),
  loading,
  refreshAll: vi.fn().mockResolvedValue(undefined),
  refreshAgents: vi.fn().mockResolvedValue([]),
});

interface Deferred<T> {
  promise: Promise<T>;
  resolve(value: T): void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

beforeEach(() => {
  vi.mocked(api.getSkillDetail).mockReset();
  vi.mocked(api.getSkillDetail).mockImplementation(async (identity) =>
    skillDetailFixture(identity.split(":").at(-1)),
  );
  vi.mocked(api.resolveGithubSkillSource).mockReset();
  vi.mocked(api.resolveGithubSkillSource).mockResolvedValue(resolutionFixture());
  vi.mocked(api.planSkillAssignment).mockReset();
  appMocks.useInstallState.mockReset();
  appMocks.useInstallState.mockReturnValue(installStateForApp(true));
  appMocks.useSkillsState.mockReset();
  appMocks.useSkillsState.mockReturnValue(skillsStateFixture());
  appMocks.useUpdater.mockReset();
  appMocks.useUpdater.mockReturnValue({
    phase: { kind: "idle" },
    checkNow: vi.fn().mockResolvedValue("latest"),
    download: vi.fn(),
    restart: vi.fn(),
    dismiss: vi.fn(),
    later: vi.fn(),
  });
  appMocks.useCliTool.mockReset();
  appMocks.usePinnedAgents.mockReset();
  appMocks.usePinnedAgents.mockReturnValue({
    agentIds: Array.from({ length: 6 }, (_, index) => `agent-${index + 1}`),
    ready: true,
    saving: false,
    commit: vi.fn().mockResolvedValue(true),
  });
  appMocks.getVersion.mockReset();
  appMocks.getVersion.mockResolvedValue("1.2.14");
  appMocks.agentViewProps.mockReset();
});

afterEach(cleanup);

describe("SkillsView", () => {
  it("renders the app-owned inventory inside the Skills workspace", () => {
    render(<SkillsView state={skillsStateFixture()} />);

    expect(
      screen.getByRole("heading", { name: "review-changes" }),
    ).toBeVisible();
    expect(screen.getByRole("tablist", { name: "Skill 状态" })).toBeVisible();
    expect(screen.getByPlaceholderText("搜索 Skills")).toBeVisible();
  });

  it("omits inferred content-type navigation", () => {
    render(<SkillsView state={skillsStateFixture()} />);

    expect(screen.queryByText("内容类型")).not.toBeInTheDocument();
    expect(screen.getByText("来源")).toBeVisible();
    expect(
      screen.queryByRole("button", { name: /说明型/ }),
    ).not.toBeInTheDocument();
  });

  it("combines filters and recomputes each axis count in the active context", async () => {
    const user = userEvent.setup();
    const inventory = skillsInventoryFixture();
    inventory.items.push(importedItem());
    render(<SkillsView state={stateWith(inventory)} />);

    await user.click(screen.getByRole("button", { name: /本地\s*2/ }));
    expect(screen.getByRole("heading", { name: "unassigned-skill" })).toBeVisible();
    expect(screen.getByRole("heading", { name: "imported-legacy" })).toBeVisible();
    expect(screen.queryByRole("heading", { name: "review-changes" })).not.toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /全部\s*2/ })).toBeVisible();

    await user.type(screen.getByPlaceholderText("搜索 Skills"), "review");
    expect(screen.getByRole("button", { name: /GitHub\s*1/ })).toBeVisible();
    expect(screen.getByRole("button", { name: /本地\s*0/ })).toBeVisible();
    expect(screen.getByText("没有匹配项")).toBeVisible();
  });

  it("opens a selected card from the keyboard and loads detail only for the inspector", async () => {
    render(<SkillsView state={skillsStateFixture()} />);
    expect(api.getSkillDetail).not.toHaveBeenCalled();

    const card = screen.getByRole("button", { name: /review-changes/ });
    expect(card).toHaveAttribute("aria-pressed", "false");
    fireEvent.keyDown(card, { key: " " });

    expect(card).toHaveAttribute("aria-pressed", "true");
    expect(await screen.findByLabelText("review-changes 详情")).toBeVisible();
    await waitFor(() =>
      expect(api.getSkillDetail).toHaveBeenCalledWith("central:review-changes"),
    );
    expect(await screen.findByLabelText("SKILL.md 纯文本预览")).toBeVisible();
  });

  it("carries the exact shared assignment loss from the Inspector into review", async () => {
    const user = userEvent.setup();
    const plan = sharedTargetPlanFixture();
    plan.kind = "assignment";
    plan.operation_id = "assignment-shared-disable";
    plan.targets[0].expected = "managed";
    plan.targets.push({
      target_id: "claude-user",
      global_dir: "~/.claude/skills",
      expected: "managed",
      primary_agent_ids: ["claude-code"],
      affected_agent_ids: ["claude-code"],
    });
    vi.mocked(api.planSkillAssignment).mockResolvedValue(plan);

    const { rerender } = render(<SkillsView state={skillsStateFixture()} />);
    await user.click(screen.getByRole("button", { name: /review-changes/ }));
    const inspector = await screen.findByLabelText("review-changes 详情");
    await user.click(
      within(inspector).getByRole("switch", { name: "停用 Cursor" }),
    );

    expect(api.planSkillAssignment).toHaveBeenCalledWith({
      skill_name: "review-changes",
      agent_ids: ["cursor"],
      enabled: false,
    });
    const impact = await screen.findByRole("region", { name: "分配影响" });
    expect(within(impact).getByText("将停止为 Cursor 分配")).toBeVisible();
    expect(within(impact).getByText("~/.agents/skills")).toBeVisible();
    expect(
      within(impact).getByText("Codex、Cursor、Gemini CLI 将失去访问"),
    ).toBeVisible();
    expect(within(impact).queryByText("~/.claude/skills")).not.toBeInTheDocument();

    const refreshed = skillsInventoryFixture();
    refreshed.items = refreshed.items.filter(
      (item) => item.name !== "review-changes",
    );
    rerender(<SkillsView state={stateWith(refreshed)} />);
    await waitFor(() =>
      expect(
        screen.queryByLabelText("review-changes 详情"),
      ).not.toBeInTheDocument(),
    );
    const retainedImpact = screen.getByRole("region", { name: "分配影响" });
    expect(
      within(retainedImpact).getByText(
        "Codex、Cursor、Gemini CLI 将失去访问",
      ),
    ).toBeVisible();
  });

  it("moves focus into the inspector and returns it to the selected card", async () => {
    const user = userEvent.setup();
    render(<SkillsView state={skillsStateFixture()} />);

    const card = screen.getByRole("button", { name: /review-changes/ });
    card.focus();
    await user.keyboard(" ");
    const inspector = await screen.findByLabelText("review-changes 详情");
    await waitFor(() => expect(inspector).toHaveFocus());

    await user.click(
      within(inspector).getByRole("button", { name: "关闭详情" }),
    );
    await waitFor(() => expect(card).toHaveFocus());
  });

  it("discards a late detail after filtering closes the inspector and another item opens", async () => {
    const user = userEvent.setup();
    const first = deferred<SkillDetail>();
    const second = {
      ...skillDetailFixture("unassigned-skill"),
      skill_md: "second-selection",
    };
    vi.mocked(api.getSkillDetail)
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(second);
    render(<SkillsView state={skillsStateFixture()} />);

    await user.click(screen.getByRole("button", { name: /review-changes/ }));
    const firstInspector = await screen.findByLabelText("review-changes 详情");
    await waitFor(() => expect(firstInspector).toHaveFocus());
    await user.type(screen.getByPlaceholderText("搜索 Skills"), "unassigned");
    await waitFor(() =>
      expect(
        screen.queryByLabelText("review-changes 详情"),
      ).not.toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /unassigned-skill/ }));
    expect(await screen.findByText("second-selection")).toBeVisible();

    first.resolve({ ...skillDetailFixture("review-changes"), skill_md: "stale-first" });
    await act(async () => {
      await first.promise;
    });
    expect(screen.queryByText("stale-first")).not.toBeInTheDocument();
    expect(screen.getByText("second-selection")).toBeVisible();
  });

  it("invalidates a pending detail request when the workspace unmounts", async () => {
    const pending = deferred<SkillDetail>();
    vi.mocked(api.getSkillDetail).mockReturnValueOnce(pending.promise);
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { unmount } = render(<SkillsView state={skillsStateFixture()} />);

    await userEvent.click(screen.getByRole("button", { name: /review-changes/ }));
    unmount();
    pending.resolve(skillDetailFixture("review-changes"));
    await act(async () => {
      await pending.promise;
    });

    expect(errorSpy).not.toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it("discards an old response when the same Skill is closed and reopened", async () => {
    const user = userEvent.setup();
    const first = deferred<SkillDetail>();
    const second = {
      ...skillDetailFixture("review-changes"),
      skill_md: "fresh-same-selection",
    };
    vi.mocked(api.getSkillDetail)
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(second);
    render(<SkillsView state={skillsStateFixture()} />);

    const card = screen.getByRole("button", { name: /review-changes/ });
    await user.click(card);
    const firstInspector = await screen.findByLabelText("review-changes 详情");
    await user.click(
      within(firstInspector).getByRole("button", { name: "关闭详情" }),
    );
    await user.click(card);
    expect(await screen.findByText("fresh-same-selection")).toBeVisible();

    first.resolve({
      ...skillDetailFixture("review-changes"),
      skill_md: "stale-same-selection",
    });
    await act(async () => {
      await first.promise;
    });
    expect(screen.queryByText("stale-same-selection")).not.toBeInTheDocument();
    expect(screen.getByText("fresh-same-selection")).toBeVisible();
  });

  it("keeps workspace chrome visible across loading, initial error, and recovery", async () => {
    const refresh = vi.fn().mockResolvedValue(skillsInventoryFixture());
    const { rerender } = render(
      <SkillsView
        state={stateWith(null, { loading: true, error: null, refresh })}
      />,
    );
    expect(screen.getByRole("tablist", { name: "Skill 状态" })).toBeVisible();
    expect(screen.getByText("正在读取 Skills…")).toBeVisible();
    expect(screen.getByRole("button", { name: "检查更新" })).toBeDisabled();

    rerender(
      <SkillsView
        state={stateWith(null, {
          loading: false,
          error: { code: "io", message: "读取失败，请检查目录权限。" },
          refresh,
        })}
      />,
    );
    expect(screen.getByText("读取 Skills 失败")).toBeVisible();
    expect(screen.getByText("读取失败，请检查目录权限。")).toBeVisible();
    await userEvent.click(screen.getByRole("button", { name: "重试" }));
    expect(refresh).toHaveBeenCalledOnce();

    const recovery = skillsInventoryFixture();
    recovery.recovery_error = "检测到未完成事务，请先恢复。";
    rerender(
      <SkillsView state={stateWith(recovery, { loading: false, error: null })} />,
    );
    expect(screen.getByText("Skills 已进入只读恢复状态")).toBeVisible();
    expect(screen.getByText("检测到未完成事务，请先恢复。")).toBeVisible();
    expect(screen.getByRole("heading", { name: "review-changes" })).toBeVisible();
    expect(screen.getByRole("button", { name: "检查更新" })).toBeDisabled();
  });

  it("treats an app-owned recovery error as read-only and consumes a blocked install intent", async () => {
    const refresh = vi.fn().mockResolvedValue(skillsInventoryFixture());
    const onIntentConsumed = vi.fn();
    render(
      <SkillsView
        state={stateWith(null, {
          loading: false,
          error: {
            code: "recovery_required",
            message: "journal recovery required",
          },
          refresh,
        })}
        intent={{ id: 41, kind: "install", agentId: "codex" }}
        onIntentConsumed={onIntentConsumed}
      />,
    );

    expect(screen.getByText("Skills 已进入只读恢复状态")).toBeVisible();
    expect(screen.getByText("journal recovery required")).toBeVisible();
    expect(screen.queryByRole("button", { name: "重试" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "检查更新" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "安装 Skill" })).toBeDisabled();
    expect(screen.queryByRole("dialog", { name: "安装 Skill" })).not.toBeInTheDocument();
    expect(onIntentConsumed).toHaveBeenCalledOnce();
    expect(onIntentConsumed).toHaveBeenCalledWith(41);
    expect(refresh).not.toHaveBeenCalled();
  });

  it("closes and cancels a resolved install when the workspace enters recovery", async () => {
    const user = userEvent.setup();
    const inventory = skillsInventoryFixture();
    const cancel = vi.fn().mockResolvedValue(undefined);
    const initialState = stateWith(inventory, { cancel });
    const { rerender } = render(
      <SkillsView
        state={initialState}
        intent={{ id: 42, kind: "install", agentId: "codex" }}
        onIntentConsumed={vi.fn()}
      />,
    );

    await user.type(screen.getByLabelText("GitHub 来源"), "acme/skills");
    await user.click(screen.getByRole("button", { name: "解析来源" }));
    expect(
      await screen.findByRole("heading", { name: "选择 Skills 与 Agent" }),
    ).toBeVisible();

    rerender(
      <SkillsView
        state={stateWith(inventory, {
          cancel,
          error: {
            code: "recovery_required",
            message: "journal recovery required",
          },
        })}
      />,
    );

    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "安装 Skill" }),
      ).not.toBeInTheDocument(),
    );
    await waitFor(() => expect(cancel).toHaveBeenCalledWith("resolve-fixture"));
    expect(cancel).toHaveBeenCalledOnce();
    expect(screen.getByText("Skills 已进入只读恢复状态")).toBeVisible();
  });

  it("keeps cached inventory under hook errors and runs only a manual metadata check", async () => {
    const pending = deferred<UpdateCheckOutcome>();
    const checkUpdates = vi.fn(() => pending.promise);
    render(
      <SkillsView
        state={stateWith(skillsInventoryFixture(), {
          error: { code: "rate_limited", message: "GitHub 暂时限流。", retry_at: "2026-07-17T08:00:00Z" },
          checkUpdates,
        })}
      />,
    );

    expect(screen.getByRole("heading", { name: "review-changes" })).toBeVisible();
    expect(screen.getByText("GitHub 暂时限流。")).toBeVisible();
    expect(screen.getByText(/2026-07-17T08:00:00Z/)).toBeVisible();
    await userEvent.click(screen.getByRole("button", { name: "检查更新" }));
    expect(checkUpdates).toHaveBeenCalledWith(true);
    expect(screen.getByRole("button", { name: "检查中…" })).toBeDisabled();

    pending.resolve({
      performed: true,
      checked: 2,
      available: [],
      skipped_pinned: [],
      errors: {},
      checked_at: "2026-07-17T08:00:00Z",
    });
    await act(async () => {
      await pending.promise;
    });
    expect(screen.getByRole("button", { name: "检查更新" })).toBeEnabled();
  });

  it("settles a pending metadata check safely after the workspace unmounts", async () => {
    const pending = deferred<UpdateCheckOutcome>();
    const checkUpdates = vi.fn(() => pending.promise);
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { unmount } = render(
      <SkillsView
        state={stateWith(skillsInventoryFixture(), { checkUpdates })}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "检查更新" }));
    unmount();
    pending.resolve({
      performed: true,
      checked: 2,
      available: [],
      skipped_pinned: [],
      errors: {},
      checked_at: "2026-07-17T08:00:00Z",
    });
    await act(async () => {
      await pending.promise;
    });

    expect(errorSpy).not.toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it("distinguishes an empty inventory from filters with no matches", async () => {
    const empty = skillsInventoryFixture();
    empty.items = [];
    const { rerender } = render(<SkillsView state={stateWith(empty)} />);
    expect(screen.getByText("暂无 Skills")).toBeVisible();
    expect(screen.getAllByRole("button", { name: "安装 Skill" })).toHaveLength(1);

    rerender(<SkillsView state={skillsStateFixture()} />);
    await userEvent.type(screen.getByPlaceholderText("搜索 Skills"), "no-such-skill");
    expect(screen.getByText("没有匹配项")).toBeVisible();
  });

  it("waits for inventory, consumes a detail intent once, and opens only a managed central Skill", async () => {
    const onIntentConsumed = vi.fn();
    const intent = { id: 17, kind: "detail" as const, skillName: "review-changes" };
    const { rerender } = render(
      <SkillsView
        state={stateWith(null, { loading: true })}
        intent={intent}
        onIntentConsumed={onIntentConsumed}
      />,
    );

    expect(onIntentConsumed).not.toHaveBeenCalled();
    expect(screen.queryByLabelText("review-changes 详情")).not.toBeInTheDocument();

    rerender(
      <SkillsView
        state={skillsStateFixture()}
        intent={intent}
        onIntentConsumed={onIntentConsumed}
      />,
    );
    expect(await screen.findByLabelText("review-changes 详情")).toBeVisible();
    expect(onIntentConsumed).toHaveBeenCalledOnce();
    expect(onIntentConsumed).toHaveBeenCalledWith(17);

    rerender(
      <SkillsView
        state={stateWith(skillsInventoryFixture())}
        intent={intent}
        onIntentConsumed={onIntentConsumed}
      />,
    );
    expect(onIntentConsumed).toHaveBeenCalledOnce();
  });

  it.each([
    ["missing-skill", "missing-skill"],
    ["external-copy", "external-copy"],
  ])("consumes an unavailable detail intent for %s with a visible notice", async (skillName) => {
    const inventory = skillsInventoryFixture();
    inventory.items.push({
      ...inventory.items[1],
      identity: "central:external-copy",
      name: "external-copy",
      states: ["external"],
      source: null,
    });
    const onIntentConsumed = vi.fn();

    render(
      <SkillsView
        state={stateWith(inventory)}
        intent={{ id: 23, kind: "detail", skillName }}
        onIntentConsumed={onIntentConsumed}
      />,
    );

    expect(
      await screen.findByText(`未找到可管理的 Skill“${skillName}”。`),
    ).toBeVisible();
    expect(onIntentConsumed).toHaveBeenCalledOnce();
    expect(screen.queryByLabelText(`${skillName} 详情`)).not.toBeInTheDocument();
  });

  it.each(["missing", "broken_link"] as const)(
    "opens a central managed source whose current target state is %s",
    async (targetState) => {
      const inventory = skillsInventoryFixture();
      const skillName = `${targetState}-central-skill`;
      inventory.items.push({
        ...inventory.items[1],
        identity: `central:${skillName}`,
        name: skillName,
        states: [targetState],
      });
      const onIntentConsumed = vi.fn();

      render(
        <SkillsView
          state={stateWith(inventory)}
          intent={{ id: 29, kind: "detail", skillName }}
          onIntentConsumed={onIntentConsumed}
        />,
      );

      expect(await screen.findByLabelText(`${skillName} 详情`)).toBeVisible();
      expect(onIntentConsumed).toHaveBeenCalledWith(29);
      expect(
        screen.queryByText(`未找到可管理的 Skill“${skillName}”。`),
      ).not.toBeInTheDocument();
    },
  );

  it("opens a source-less central anomaly referenced by assignment settings", async () => {
    const inventory = skillsInventoryFixture();
    inventory.items.push({
      ...inventory.items[1],
      identity: "central:assigned-external-anomaly",
      name: "assigned-external-anomaly",
      states: ["external"],
      source: null,
      assigned_target_ids: ["agents-user"],
      affected_agent_ids: ["codex", "cursor", "gemini"],
    });
    const onIntentConsumed = vi.fn();

    render(
      <SkillsView
        state={stateWith(inventory)}
        intent={{
          id: 30,
          kind: "detail",
          skillName: "assigned-external-anomaly",
        }}
        onIntentConsumed={onIntentConsumed}
      />,
    );

    expect(
      await screen.findByLabelText("assigned-external-anomaly 详情"),
    ).toBeVisible();
    expect(onIntentConsumed).toHaveBeenCalledWith(30);
  });

  it("clears an unavailable navigation notice after a manual Skill selection", async () => {
    render(
      <SkillsView
        state={skillsStateFixture()}
        intent={{ id: 30, kind: "detail", skillName: "missing-skill" }}
        onIntentConsumed={vi.fn()}
      />,
    );
    expect(
      await screen.findByText("未找到可管理的 Skill“missing-skill”。"),
    ).toBeVisible();

    await userEvent.click(screen.getByRole("button", { name: /review-changes/ }));
    expect(await screen.findByLabelText("review-changes 详情")).toBeVisible();
    expect(
      screen.queryByText("未找到可管理的 Skill“missing-skill”。"),
    ).not.toBeInTheDocument();
  });

  it("consumes each new install intent once and opens the shared install dialog", async () => {
    const onIntentConsumed = vi.fn();
    const { rerender } = render(
      <SkillsView
        state={skillsStateFixture()}
        intent={{ id: 31, kind: "install", agentId: "codex" }}
        onIntentConsumed={onIntentConsumed}
      />,
    );

    expect(await screen.findByRole("dialog", { name: "安装 Skill" })).toBeVisible();
    expect(onIntentConsumed).toHaveBeenCalledWith(31);
    await userEvent.click(screen.getByRole("button", { name: "关闭安装" }));

    rerender(
      <SkillsView
        state={skillsStateFixture()}
        intent={{ id: 32, kind: "install", agentId: "cursor" }}
        onIntentConsumed={onIntentConsumed}
      />,
    );
    expect(await screen.findByRole("dialog", { name: "安装 Skill" })).toBeVisible();
    expect(onIntentConsumed).toHaveBeenCalledTimes(2);
    expect(onIntentConsumed).toHaveBeenLastCalledWith(32);
  });
});

describe("App Skills routing", () => {
  it("opens Skills before the MCP loading gate with one app-owned state", async () => {
    render(<App />);
    expect(screen.getByText("加载中…")).toBeVisible();
    expect(screen.getByRole("button", { name: "MCPs" })).toBeVisible();
    expect(screen.getByRole("button", { name: /Models/ })).toBeVisible();
    expect(screen.getByRole("navigation", { name: "置顶 Agent" })).toBeVisible();
    expect(appMocks.useSkillsState).toHaveBeenCalledOnce();

    await userEvent.click(screen.getByRole("button", { name: "Skills" }));
    expect(screen.getByRole("heading", { name: "review-changes" })).toBeVisible();
    expect(screen.queryByText("加载中…")).not.toBeInTheDocument();
    expect(appMocks.useSkillsState).toHaveBeenCalledTimes(2);
  });

  it("keeps MCP, Models, and Agent routes explicit after adding Skills", async () => {
    const user = userEvent.setup();
    appMocks.useInstallState.mockReturnValue(installStateForApp(false));
    render(<App />);

    expect(screen.getByText("registry-view")).toBeVisible();
    await user.click(screen.getByRole("button", { name: /Models/ }));
    expect(screen.getByText("models-view")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "MCPs" }));
    expect(screen.getByText("registry-view")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Agent 1" }));
    expect(screen.getByText("agent-view:agent-1")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Skills" }));
    expect(screen.getByRole("heading", { name: "review-changes" })).toBeVisible();
  });

  it("uses two Escape presses to close an Agent picker above a Skill inspector", async () => {
    const user = userEvent.setup();
    appMocks.useInstallState.mockReturnValue(installStateForApp(false));
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Skills" }));
    await user.click(screen.getByRole("button", { name: /review-changes/ }));
    expect(await screen.findByLabelText("review-changes 详情")).toBeVisible();

    await user.click(screen.getByRole("button", { name: "选择 Agent" }));
    expect(screen.getByRole("dialog", { name: "选择和置顶 Agent" })).toBeVisible();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "选择和置顶 Agent" })).not.toBeInTheDocument();
    expect(screen.getByLabelText("review-changes 详情")).toBeVisible();

    await user.keyboard("{Escape}");
    expect(screen.queryByLabelText("review-changes 详情")).not.toBeInTheDocument();
  });

  it("routes typed Agent requests through the sole app-owned Skills state", async () => {
    const user = userEvent.setup();
    const skillsState = skillsStateFixture();
    appMocks.useInstallState.mockReturnValue(installStateForApp(false));
    appMocks.useSkillsState.mockReturnValue(skillsState);
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Agent 1" }));
    expect(appMocks.agentViewProps).toHaveBeenLastCalledWith(
      expect.objectContaining({
        agentId: "agent-1",
        skillsState,
        onOpenSkills: expect.any(Function),
      }),
    );
    const openSkills = appMocks.agentViewProps.mock.calls.at(-1)?.[0]
      .onOpenSkills as (request: {
        kind: "detail";
        skillName: string;
      }) => void;

    await user.click(screen.getByRole("button", { name: "查看 Agent Skill" }));
    const inspector = await screen.findByLabelText("review-changes 详情");
    expect(inspector).toBeVisible();
    await user.click(
      within(inspector).getByRole("button", { name: "关闭详情" }),
    );
    expect(screen.queryByLabelText("review-changes 详情")).not.toBeInTheDocument();

    act(() => openSkills({ kind: "detail", skillName: "review-changes" }));
    expect(await screen.findByLabelText("review-changes 详情")).toBeVisible();
    expect(
      appMocks.useSkillsState.mock.results
        .filter((result) => result.type === "return")
        .every((result) => result.value === skillsState),
    ).toBe(true);
  });
});
