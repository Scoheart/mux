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
import type { SkillsState } from "../hooks/useSkillsState";
import * as api from "../lib/api";
import type {
  OperationPlan,
  SkillInventoryItem,
  SkillSourceResolution,
  SkillsInventory,
} from "../lib/types";
import {
  agentFixture,
  resolutionFixture,
  sharedTargetPlanFixture,
  skillsInventoryFixture,
  skillsStateFixture,
} from "../test/skillsFixtures";
import { ToastProvider } from "./Toast";
import { SkillInstallDialog } from "./SkillInstallDialog";
import { SkillsView } from "./SkillsView";

const dialogCss = await readFile(resolve(process.cwd(), "src/index.css"), "utf8");

function groupedDeclarations(source: string, selector: string): string | null {
  const uncommented = source.replace(/\/\*[\s\S]*?\*\//g, "");
  for (const match of uncommented.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const selectors = match[1]
      .split(",")
      .map((candidate) => candidate.trim());
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
    getSkillDetail: vi.fn(),
    resolveGithubSkillSource: vi.fn(),
    resolveLocalSkillSourceDialog: vi.fn(),
    planSkillInstall: vi.fn(),
    planSkillImport: vi.fn(),
    planSkillUpdate: vi.fn(),
    planSkillRemove: vi.fn(),
    planSkillAssignment: vi.fn(),
    planSkillRepair: vi.fn(),
  };
});

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

const twoCandidateResolution = (): SkillSourceResolution => {
  const resolution = resolutionFixture();
  return {
    ...resolution,
    candidates: [
      ...resolution.candidates,
      {
        ...resolution.candidates[0],
        name: "release-notes",
        description: "Prepare release notes",
        relative_path: "release-notes",
        content_hash: "content-release",
      },
    ],
  };
};

const planAs = (kind: OperationPlan["kind"], operationId = `${kind}-operation`) => ({
  ...sharedTargetPlanFixture(),
  operation_id: operationId,
  kind,
});

function renderInstall(overrides: {
  agents?: ReturnType<typeof agentFixture>;
  initialAgentId?: string;
  commit?: SkillsState["commit"];
  cancel?: SkillsState["cancel"];
  onClose?: () => void;
  onCommitted?: (inventory: SkillsInventory) => void;
  onRecoveryRequired?: (message: string) => void;
} = {}) {
  const state = skillsStateFixture();
  const props = {
    agents: overrides.agents ?? agentFixture(),
    initialAgentId: overrides.initialAgentId,
    commit: overrides.commit ?? state.commit,
    cancel: overrides.cancel ?? state.cancel,
    onClose: overrides.onClose ?? vi.fn(),
    onCommitted: overrides.onCommitted ?? vi.fn(),
    onRecoveryRequired: overrides.onRecoveryRequired ?? vi.fn(),
  };
  const rendered = render(
    <ToastProvider>
      <SkillInstallDialog {...props} />
    </ToastProvider>,
  );
  return { ...rendered, props };
}

async function resolveGithub(
  user: ReturnType<typeof userEvent.setup>,
  value = "acme/skills",
) {
  await user.type(screen.getByLabelText("GitHub 来源"), value);
  await user.click(screen.getByRole("button", { name: "解析来源" }));
  await screen.findByRole("heading", { name: "选择 Skills 与 Agent" });
}

beforeEach(() => {
  vi.mocked(api.resolveGithubSkillSource).mockResolvedValue(resolutionFixture());
  vi.mocked(api.resolveLocalSkillSourceDialog).mockResolvedValue(null);
  vi.mocked(api.planSkillInstall).mockResolvedValue(sharedTargetPlanFixture());
  vi.mocked(api.planSkillImport).mockResolvedValue(planAs("import"));
  vi.mocked(api.planSkillUpdate).mockResolvedValue(planAs("update"));
  vi.mocked(api.planSkillRemove).mockResolvedValue(planAs("remove"));
  vi.mocked(api.planSkillAssignment).mockResolvedValue(planAs("assignment"));
  vi.mocked(api.planSkillRepair).mockResolvedValue(planAs("repair"));
  vi.mocked(api.getSkillDetail).mockImplementation(async (identity) => ({
    item:
      skillsInventoryFixture().items.find((item) => item.identity === identity) ??
      skillsInventoryFixture().items[0],
    files: [],
    skill_md: "---\nname: fixture\n---\n",
    skill_md_truncated: false,
  }));
});

afterEach(cleanup);

describe("SkillInstallDialog", () => {
  it("trims GitHub input, exposes busy/errors, selects all candidates and zero Agents", async () => {
    const user = userEvent.setup();
    const pending = deferred<SkillSourceResolution>();
    vi.mocked(api.resolveGithubSkillSource).mockReturnValueOnce(pending.promise);
    renderInstall();

    await user.type(screen.getByLabelText("GitHub 来源"), "  acme/skills  ");
    await user.click(screen.getByRole("button", { name: "解析来源" }));
    expect(api.resolveGithubSkillSource).toHaveBeenCalledWith("acme/skills");
    expect(screen.getByRole("button", { name: "解析中…" })).toBeDisabled();

    pending.resolve(twoCandidateResolution());
    expect(
      await screen.findByRole("heading", { name: "选择 Skills 与 Agent" }),
    ).toBeVisible();
    expect(screen.getByRole("checkbox", { name: "review-changes" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "release-notes" })).toBeChecked();
    for (const name of [
      "Claude Code",
      "Codex",
      "Cursor",
      "Gemini CLI",
      "OpenCode",
      "GitHub Copilot CLI",
    ]) {
      expect(screen.getByRole("checkbox", { name })).not.toBeChecked();
    }

    await user.click(screen.getByRole("button", { name: "返回来源" }));
    vi.mocked(api.resolveGithubSkillSource).mockRejectedValueOnce({
      code: "rate_limited",
      message: "GitHub 暂时限流。",
      retry_at: "2026-07-17T08:00:00Z",
    });
    await user.click(screen.getByRole("button", { name: "解析来源" }));
    expect(await screen.findByText("GitHub 暂时限流。")).toBeVisible();
    expect(screen.getByText(/2026-07-17T08:00:00Z/)).toBeVisible();
  });

  it("preselects only the exact verified Agent once per source resolution", async () => {
    const user = userEvent.setup();
    renderInstall({ initialAgentId: "codex" });
    await resolveGithub(user);

    const codex = screen.getByRole("checkbox", { name: "Codex" });
    expect(codex).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Cursor" })).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Gemini CLI" })).not.toBeChecked();

    await user.click(codex);
    await user.click(screen.getByRole("checkbox", { name: "review-changes" }));
    await user.click(screen.getByRole("checkbox", { name: "review-changes" }));
    expect(codex).not.toBeChecked();

    await user.click(screen.getByRole("button", { name: "返回来源" }));
    await screen.findByRole("heading", { name: "安装 Skill" });
    await user.click(screen.getByRole("button", { name: "解析来源" }));
    await screen.findByRole("heading", { name: "选择 Skills 与 Agent" });
    expect(screen.getByRole("checkbox", { name: "Codex" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Cursor" })).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Gemini CLI" })).not.toBeChecked();
  });

  it("ignores an initial Agent that is absent from verified inventory", async () => {
    const user = userEvent.setup();
    renderInstall({ initialAgentId: "unknown-agent" });
    await resolveGithub(user);

    for (const agent of agentFixture()) {
      expect(screen.getByRole("checkbox", { name: agent.name })).not.toBeChecked();
    }
  });

  it("drops a preselected Agent removed while source resolution is pending", async () => {
    const user = userEvent.setup();
    const pending = deferred<SkillSourceResolution>();
    vi.mocked(api.resolveGithubSkillSource).mockReturnValueOnce(pending.promise);
    const rendered = renderInstall({ initialAgentId: "codex" });

    await user.type(screen.getByLabelText("GitHub 来源"), "acme/skills");
    await user.click(screen.getByRole("button", { name: "解析来源" }));
    const remainingAgents = agentFixture().filter((agent) => agent.id !== "codex");
    rendered.rerender(
      <ToastProvider>
        <SkillInstallDialog
          {...rendered.props}
          agents={remainingAgents}
          initialAgentId="codex"
        />
      </ToastProvider>,
    );

    pending.resolve(resolutionFixture());
    await screen.findByRole("heading", { name: "选择 Skills 与 Agent" });
    expect(screen.queryByRole("checkbox", { name: "Codex" })).not.toBeInTheDocument();
    for (const agent of remainingAgents) {
      expect(screen.getByRole("checkbox", { name: agent.name })).not.toBeChecked();
    }

    await user.click(screen.getByRole("button", { name: "审阅安装" }));
    expect(api.planSkillInstall).toHaveBeenCalledWith(
      expect.objectContaining({ agent_ids: [] }),
    );
  });

  it("uses the latest verified initial Agent when intent changes during resolution", async () => {
    const user = userEvent.setup();
    const pending = deferred<SkillSourceResolution>();
    vi.mocked(api.resolveGithubSkillSource).mockReturnValueOnce(pending.promise);
    const rendered = renderInstall({ initialAgentId: "codex" });

    await user.type(screen.getByLabelText("GitHub 来源"), "acme/skills");
    await user.click(screen.getByRole("button", { name: "解析来源" }));
    rendered.rerender(
      <ToastProvider>
        <SkillInstallDialog
          {...rendered.props}
          initialAgentId="cursor"
        />
      </ToastProvider>,
    );

    pending.resolve(resolutionFixture());
    await screen.findByRole("heading", { name: "选择 Skills 与 Agent" });
    expect(screen.getByRole("checkbox", { name: "Cursor" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Codex" })).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Gemini CLI" })).not.toBeChecked();
  });

  it("uses one verified selection for count, plan, and shared impact after an Agent disappears", async () => {
    const user = userEvent.setup();
    const rendered = renderInstall();
    await resolveGithub(user);
    await user.click(screen.getByRole("checkbox", { name: "Codex" }));

    const remainingAgents = agentFixture().filter((agent) => agent.id !== "codex");
    rendered.rerender(
      <ToastProvider>
        <SkillInstallDialog
          {...rendered.props}
          agents={remainingAgents}
        />
      </ToastProvider>,
    );
    expect(screen.queryByRole("checkbox", { name: "Codex" })).not.toBeInTheDocument();
    const targetSection = screen
      .getByRole("heading", { name: "目标 Agent" })
      .closest("section");
    expect(targetSection).not.toBeNull();
    expect(within(targetSection!).getByText("0 个")).toBeVisible();

    await user.click(screen.getByRole("button", { name: "审阅安装" }));
    expect(api.planSkillInstall).toHaveBeenCalledWith(
      expect.objectContaining({ agent_ids: [] }),
    );
    const impact = await screen.findByRole("region", { name: "共享目标影响" });
    expect(within(impact).getByText(/codex、Cursor、Gemini CLI/)).toBeVisible();
  });

  it("does not resurrect a removed Agent selection when that Agent reappears", async () => {
    const user = userEvent.setup();
    const rendered = renderInstall();
    const allAgents = agentFixture();
    await resolveGithub(user);
    await user.click(screen.getByRole("checkbox", { name: "Codex" }));

    rendered.rerender(
      <ToastProvider>
        <SkillInstallDialog
          {...rendered.props}
          agents={allAgents.filter((agent) => agent.id !== "codex")}
        />
      </ToastProvider>,
    );
    const targetSection = screen
      .getByRole("heading", { name: "目标 Agent" })
      .closest("section");
    expect(targetSection).not.toBeNull();
    await waitFor(() =>
      expect(within(targetSection!).getByText("0 个")).toBeVisible(),
    );

    rendered.rerender(
      <ToastProvider>
        <SkillInstallDialog {...rendered.props} agents={allAgents} />
      </ToastProvider>,
    );
    expect(screen.getByRole("checkbox", { name: "Codex" })).not.toBeChecked();
  });

  it("uses only the native local picker and leaves state untouched on picker cancel", async () => {
    const user = userEvent.setup();
    renderInstall();

    expect(screen.queryByLabelText("本地路径")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "选择本地文件夹" }));
    expect(api.resolveLocalSkillSourceDialog).toHaveBeenCalledOnce();
    expect(screen.getByRole("heading", { name: "安装 Skill" })).toBeVisible();
    expect(screen.queryByRole("heading", { name: "选择 Skills 与 Agent" })).not.toBeInTheDocument();
  });

  it("plans selected Skills and Agents into the resolution operation and shows shared impact", async () => {
    const user = userEvent.setup();
    vi.mocked(api.resolveGithubSkillSource).mockResolvedValueOnce(
      twoCandidateResolution(),
    );
    renderInstall();
    await resolveGithub(user);

    await user.click(screen.getByRole("checkbox", { name: "release-notes" }));
    await user.click(screen.getByRole("checkbox", { name: "Codex" }));
    await user.click(screen.getByRole("checkbox", { name: "Cursor" }));
    await user.click(screen.getByRole("button", { name: "审阅安装" }));

    expect(api.planSkillInstall).toHaveBeenCalledWith({
      resolution_id: "resolve-fixture",
      skill_names: ["review-changes"],
      agent_ids: ["codex", "cursor"],
      replace_conflicts: false,
    });
    const impact = await screen.findByRole("region", { name: "共享目标影响" });
    expect(within(impact).getByText("~/.agents/skills")).toBeVisible();
    expect(within(impact).getByText(/也会被 Gemini CLI 读取/)).toBeVisible();
  });

  it("keeps central replacement unchecked and replans after explicit opt-in", async () => {
    const user = userEvent.setup();
    renderInstall();
    await resolveGithub(user);

    const replacement = screen.getByRole("checkbox", {
      name: "备份并替换同名中央副本",
    });
    expect(replacement).not.toBeChecked();
    await user.click(screen.getByRole("button", { name: "审阅安装" }));
    expect(api.planSkillInstall).toHaveBeenLastCalledWith(
      expect.objectContaining({ replace_conflicts: false }),
    );
    await user.keyboard("{Escape}");

    await user.click(replacement);
    expect(replacement).toBeChecked();
    expect(
      screen.queryByRole("dialog", { name: "审阅 Skill 操作" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "审阅安装" }));
    expect(api.planSkillInstall).toHaveBeenLastCalledWith(
      expect.objectContaining({ replace_conflicts: true }),
    );
    expect(api.planSkillInstall).toHaveBeenCalledTimes(2);
  });

  it("replans selection in the same operation without cancelling staged resolution", async () => {
    const user = userEvent.setup();
    const cancel = vi.fn().mockResolvedValue(undefined);
    renderInstall({ cancel });
    await resolveGithub(user);

    await user.click(screen.getByRole("checkbox", { name: "Codex" }));
    await user.click(screen.getByRole("button", { name: "审阅安装" }));
    expect(await screen.findByRole("dialog", { name: "审阅 Skill 操作" })).toBeVisible();
    await user.keyboard("{Escape}");

    await user.click(screen.getByRole("checkbox", { name: "Cursor" }));
    await user.click(screen.getByRole("button", { name: "审阅安装" }));
    expect(api.planSkillInstall).toHaveBeenCalledTimes(2);
    expect(vi.mocked(api.planSkillInstall).mock.calls[0][0].resolution_id).toBe(
      "resolve-fixture",
    );
    expect(vi.mocked(api.planSkillInstall).mock.calls[1][0].resolution_id).toBe(
      "resolve-fixture",
    );
    expect(cancel).not.toHaveBeenCalled();
  });

  it("coalesces final close, cancels once, and still closes after cancellation fails", async () => {
    const user = userEvent.setup();
    const cancellation = deferred<void>();
    const cancel = vi.fn(() => cancellation.promise);
    const onClose = vi.fn();
    renderInstall({ cancel, onClose });
    await resolveGithub(user);

    const close = screen.getByRole("button", { name: "关闭安装" });
    fireEvent.click(close);
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.click(close);
    expect(cancel).toHaveBeenCalledOnce();
    expect(cancel).toHaveBeenCalledWith("resolve-fixture");
    expect(onClose).not.toHaveBeenCalled();

    cancellation.reject({ code: "io", message: "无法清理 staging。" });
    expect(await screen.findByText("无法清理 staging。")).toBeVisible();
    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  });

  it("best-effort cancels a late resolution after close", async () => {
    const user = userEvent.setup();
    const pending = deferred<SkillSourceResolution>();
    const cancel = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    vi.mocked(api.resolveGithubSkillSource).mockReturnValueOnce(pending.promise);
    renderInstall({ cancel, onClose });

    await user.type(screen.getByLabelText("GitHub 来源"), "acme/skills");
    await user.click(screen.getByRole("button", { name: "解析来源" }));
    await user.click(screen.getByRole("button", { name: "关闭安装" }));
    expect(onClose).toHaveBeenCalledOnce();

    pending.resolve(resolutionFixture());
    await waitFor(() =>
      expect(cancel).toHaveBeenCalledWith("resolve-fixture"),
    );
    expect(screen.queryByRole("heading", { name: "选择 Skills 与 Agent" })).not.toBeInTheDocument();
  });

  it("lets close own cleanup while a late install plan is discarded", async () => {
    const user = userEvent.setup();
    const pending = deferred<OperationPlan>();
    const cancellation = deferred<void>();
    const cancel = vi.fn(() => cancellation.promise);
    vi.mocked(api.planSkillInstall).mockReturnValueOnce(pending.promise);
    renderInstall({ cancel });
    await resolveGithub(user);

    await user.click(screen.getByRole("button", { name: "审阅安装" }));
    expect(screen.getByRole("button", { name: "生成计划中…" })).toBeDisabled();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(cancel).toHaveBeenCalledOnce();
    cancellation.resolve();

    pending.resolve(sharedTargetPlanFixture());
    await act(async () => {
      await pending.promise;
    });
    expect(cancel).toHaveBeenCalledOnce();
    expect(screen.queryByRole("dialog", { name: "审阅 Skill 操作" })).not.toBeInTheDocument();
  });

  it("never races cancellation with commit and never cancels committed content", async () => {
    const user = userEvent.setup();
    const committing = deferred<SkillsInventory>();
    const commit = vi.fn(() => committing.promise);
    const cancel = vi.fn().mockResolvedValue(undefined);
    const onCommitted = vi.fn();
    renderInstall({ commit, cancel, onCommitted });
    await resolveGithub(user);
    await user.click(screen.getByRole("button", { name: "审阅安装" }));

    await user.click(await screen.findByRole("button", { name: "确认安装" }));
    fireEvent.keyDown(document, { key: "Escape" });
    expect(commit).toHaveBeenCalledWith(sharedTargetPlanFixture(), null);
    expect(cancel).not.toHaveBeenCalled();

    const inventory = skillsInventoryFixture();
    committing.resolve(inventory);
    await waitFor(() => expect(onCommitted).toHaveBeenCalledWith(inventory));
    expect(cancel).not.toHaveBeenCalled();
  });

  it("does not blindly cancel a recovery-owned install after unmount", async () => {
    const user = userEvent.setup();
    const committing = deferred<SkillsInventory>();
    const cancel = vi.fn().mockResolvedValue(undefined);
    const { unmount } = renderInstall({
      commit: vi.fn(() => committing.promise),
      cancel,
    });
    await resolveGithub(user);
    await user.click(screen.getByRole("button", { name: "审阅安装" }));
    await user.click(await screen.findByRole("button", { name: "确认安装" }));

    unmount();
    expect(cancel).not.toHaveBeenCalled();
    committing.reject({
      code: "recovery_required",
      message: "journal recovery required",
    });
    await act(async () => {
      await committing.promise.catch(() => undefined);
    });
    expect(cancel).not.toHaveBeenCalled();
  });
});

describe("Skill dialog 900 by 600 layout contract", () => {
  it("keeps opaque chrome visible while only the bounded body scrolls", () => {
    for (const selector of [
      ".mux-skill-install-dialog",
      ".mux-skill-review-dialog",
      ".mux-skill-risk-dialog",
    ]) {
      const shell = groupedDeclarations(dialogCss, selector);
      expect(shell).toMatch(/display:\s*flex/);
      expect(shell).toMatch(/min-height:\s*0/);
      expect(shell).toMatch(/flex-direction:\s*column/);
      expect(shell).not.toMatch(/position:\s*fixed/);
    }

    for (const selector of [
      ".mux-skill-dialog-body",
      ".mux-skill-review-body",
    ]) {
      const body = groupedDeclarations(dialogCss, selector);
      expect(body).toMatch(/min-height:\s*0/);
      expect(body).toMatch(/flex:\s*1\s+1\s+auto/);
      expect(body).toMatch(/overflow-y:\s*auto/);
    }

    for (const selector of [
      ".mux-skill-dialog-footer",
      ".mux-skill-review-footer",
    ]) {
      const footer = groupedDeclarations(dialogCss, selector);
      expect(footer).toMatch(/flex:\s*0\s+0\s+auto/);
      expect(footer).toMatch(/background:\s*var\(--surface-overlay\)/);
      expect(footer).not.toMatch(/position:\s*fixed/);
    }
  });

  it("compresses modal chrome at the acceptance viewport", () => {
    const narrow = mediaBlock(dialogCss, "@media (max-width: 920px)");
    const short = mediaBlock(dialogCss, "@media (max-height: 640px)");

    expect(narrow).toContain(".mux-skill-dialog-body");
    expect(narrow).toContain(".mux-skill-review-footer");
    expect(short).toContain(".mux-skill-dialog-header");
    expect(short).toContain(".mux-skill-review-body");
    expect(short).toContain(".mux-skill-review-footer");
  });
});

describe("Skills lifecycle orchestration", () => {
  const managedItem = () => skillsInventoryFixture().items[0];

  const renderWorkspace = (
    inventory: SkillsInventory,
    overrides: Partial<SkillsState> = {},
  ) => {
    const state = { ...skillsStateFixture(), inventory, ...overrides };
    return render(
      <ToastProvider>
        <SkillsView state={state} />
      </ToastProvider>,
    );
  };

  async function openInspector(item: SkillInventoryItem) {
    await userEvent.click(screen.getByRole("button", { name: new RegExp(item.name) }));
    return screen.findByLabelText(`${item.name} 详情`);
  }

  it("opens the install wizard from the toolbar and commits only through app state", async () => {
    const commit = vi.fn().mockResolvedValue(skillsInventoryFixture());
    renderWorkspace(skillsInventoryFixture(), { commit });
    await userEvent.click(screen.getByRole("button", { name: "安装 Skill" }));
    expect(screen.getByRole("dialog", { name: "安装 Skill" })).toBeVisible();
    expect(commit).not.toHaveBeenCalled();
  });

  it.each([
    {
      label: "导入",
      setup: () => {
        const inventory = skillsInventoryFixture();
        inventory.items = [
          {
            ...inventory.items[1],
            identity: "target:agents-user:external-copy",
            name: "external-copy",
            states: ["external"],
            location: {
              kind: "agent_target" as const,
              target_id: "agents-user",
              global_dir: "~/.agents/skills",
            },
            source: null,
            affected_agent_ids: ["codex", "cursor", "gemini"],
          },
        ];
        return {
          inventory,
          item: inventory.items[0],
          planner: api.planSkillImport,
          expected: {
            identity: "target:agents-user:external-copy",
            agent_ids: ["codex", "cursor", "gemini"],
            replace_conflicts: false,
          },
        };
      },
    },
    {
      label: "更新",
      setup: () => ({
        inventory: skillsInventoryFixture(),
        item: managedItem(),
        planner: api.planSkillUpdate,
        expected: { skill_name: "review-changes", replace_local_changes: false },
      }),
    },
    {
      label: "修复",
      setup: () => {
        const inventory = skillsInventoryFixture();
        inventory.items[0] = {
          ...inventory.items[0],
          states: ["missing"],
        };
        return {
          inventory,
          item: inventory.items[0],
          planner: api.planSkillRepair,
          expected: {
            skill_name: "review-changes",
            repair: { kind: "central" as const },
          },
        };
      },
    },
    {
      label: "移除",
      setup: () => ({
        inventory: skillsInventoryFixture(),
        item: managedItem(),
        planner: api.planSkillRemove,
        expected: { skill_name: "review-changes" },
      }),
    },
  ])("plans $label before injected commit", async ({ setup, label }) => {
    const user = userEvent.setup();
    const { inventory, item, planner, expected } = setup();
    const commit = vi.fn().mockResolvedValue(inventory);
    renderWorkspace(inventory, { commit });
    const inspector = await openInspector(item);

    await user.click(within(inspector).getByRole("button", { name: label }));
    expect(planner).toHaveBeenCalledWith(expected);
    expect(commit).not.toHaveBeenCalled();
    await user.click(await screen.findByRole("button", { name: /确认/ }));
    expect(commit).toHaveBeenCalledWith(
      expect.objectContaining({ kind: label === "导入" ? "import" : label === "更新" ? "update" : label === "修复" ? "repair" : "remove" }),
      null,
    );
  });

  it("sends explicit replacement choices for import and locally modified update", async () => {
    const user = userEvent.setup();
    const inventory = skillsInventoryFixture();
    const external: SkillInventoryItem = {
      ...inventory.items[1],
      identity: "target:agents-user:external-copy",
      name: "external-copy",
      states: ["external"],
      location: {
        kind: "agent_target",
        target_id: "agents-user",
        global_dir: "~/.agents/skills",
      },
      source: null,
      affected_agent_ids: ["codex", "cursor", "gemini"],
    };
    const modified: SkillInventoryItem = {
      ...managedItem(),
      states: ["locally_modified"],
    };
    inventory.items = [external, modified];
    renderWorkspace(inventory);

    let inspector = await openInspector(external);
    const importReplacement = within(inspector).getByRole("checkbox", {
      name: "备份并替换同名中央副本",
    });
    expect(importReplacement).not.toBeChecked();
    await user.click(importReplacement);
    await user.click(within(inspector).getByRole("button", { name: "导入" }));
    expect(api.planSkillImport).toHaveBeenCalledWith({
      identity: "target:agents-user:external-copy",
      agent_ids: ["codex", "cursor", "gemini"],
      replace_conflicts: true,
    });
    await user.keyboard("{Escape}");
    await user.click(within(inspector).getByRole("button", { name: "关闭详情" }));

    inspector = await openInspector(modified);
    expect(within(inspector).getByRole("button", { name: "更新" })).toBeVisible();
    const updateReplacement = within(inspector).getByRole("checkbox", {
      name: "保留备份并替换本地更改",
    });
    expect(updateReplacement).not.toBeChecked();
    await user.click(updateReplacement);
    await user.click(within(inspector).getByRole("button", { name: "更新" }));
    expect(api.planSkillUpdate).toHaveBeenCalledWith({
      skill_name: "review-changes",
      replace_local_changes: true,
    });
  });

  it.each(["broken_link", "missing"] as const)(
    "plans target repair for a real %s inventory row",
    async (state) => {
      const user = userEvent.setup();
      const inventory = skillsInventoryFixture();
      const item: SkillInventoryItem = {
        ...inventory.items[0],
        identity: "target:agents-user:review-changes",
        states: [state],
        location: {
          kind: "agent_target",
          target_id: "agents-user",
          global_dir: "~/.agents/skills",
        },
      };
      inventory.items = [item];
      const commit = vi.fn().mockResolvedValue(inventory);
      renderWorkspace(inventory, { commit });
      const inspector = await openInspector(item);

      await user.click(within(inspector).getByRole("button", { name: "修复" }));
      expect(api.planSkillRepair).toHaveBeenCalledWith({
        skill_name: "review-changes",
        repair: { kind: "target", target_id: "agents-user" },
      });
      await user.click(await screen.findByRole("button", { name: "确认修复" }));
      expect(commit).toHaveBeenCalledWith(
        expect.objectContaining({ kind: "repair" }),
        null,
      );
    },
  );

  it("plans central repair for a managed broken central inventory row", async () => {
    const user = userEvent.setup();
    const inventory = skillsInventoryFixture();
    const item: SkillInventoryItem = {
      ...inventory.items[0],
      states: ["broken_link"],
      location: { kind: "central" },
    };
    inventory.items = [item];
    renderWorkspace(inventory);
    const inspector = await openInspector(item);

    await user.click(within(inspector).getByRole("button", { name: "修复" }));
    expect(api.planSkillRepair).toHaveBeenCalledWith({
      skill_name: "review-changes",
      repair: { kind: "central" },
    });
  });

  it("plans assignment switches and shows every shared target impact before commit", async () => {
    const user = userEvent.setup();
    const inventory = skillsInventoryFixture();
    const commit = vi.fn().mockResolvedValue(inventory);
    renderWorkspace(inventory, { commit });
    const inspector = await openInspector(inventory.items[0]);

    await user.click(
      within(inspector).getByRole("switch", { name: "停用 Codex" }),
    );
    expect(api.planSkillAssignment).toHaveBeenCalledWith({
      skill_name: "review-changes",
      agent_ids: ["codex"],
      enabled: false,
    });
    const review = await screen.findByRole("dialog", { name: "审阅 Skill 操作" });
    const impact = within(review).getByRole("region", { name: "分配影响" });
    expect(within(impact).getByText("~/.agents/skills")).toBeVisible();
    expect(
      within(impact).getByText("Codex、Cursor、Gemini CLI 将失去访问"),
    ).toBeVisible();
    expect(commit).not.toHaveBeenCalled();
  });

  it("discards and cancels a late lifecycle plan after the Inspector closes", async () => {
    const user = userEvent.setup();
    const pending = deferred<OperationPlan>();
    const cancel = vi.fn().mockResolvedValue(undefined);
    vi.mocked(api.planSkillUpdate).mockReturnValueOnce(pending.promise);
    const inventory = skillsInventoryFixture();
    renderWorkspace(inventory, { cancel });
    const inspector = await openInspector(inventory.items[0]);

    await user.click(within(inspector).getByRole("button", { name: "更新" }));
    await user.click(within(inspector).getByRole("button", { name: "关闭详情" }));
    pending.resolve(planAs("update", "late-update"));
    await waitFor(() => expect(cancel).toHaveBeenCalledWith("late-update"));
    expect(screen.queryByRole("dialog", { name: "审阅 Skill 操作" })).not.toBeInTheDocument();
  });

  it("cancels an open lifecycle review once, but not after commit", async () => {
    const user = userEvent.setup();
    const inventory = skillsInventoryFixture();
    const cancel = vi.fn().mockResolvedValue(undefined);
    const commit = vi.fn().mockResolvedValue(inventory);
    render(
      <ToastProvider>
        <SkillsView state={{ ...skillsStateFixture(), inventory, cancel, commit }} />
      </ToastProvider>,
    );
    const inspector = await openInspector(inventory.items[0]);
    await user.click(within(inspector).getByRole("button", { name: "更新" }));
    await screen.findByRole("dialog", { name: "审阅 Skill 操作" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(cancel).toHaveBeenCalledOnce());

    await user.click(within(inspector).getByRole("button", { name: "更新" }));
    await user.click(await screen.findByRole("button", { name: "确认更新" }));
    await waitFor(() => expect(commit).toHaveBeenCalled());
    expect(cancel).toHaveBeenCalledOnce();
  });

  it("defers unmount cleanup until a lifecycle commit settles", async () => {
    const user = userEvent.setup();
    const inventory = skillsInventoryFixture();
    const committing = deferred<SkillsInventory>();
    const cancel = vi.fn().mockResolvedValue(undefined);
    const { unmount } = renderWorkspace(inventory, {
      cancel,
      commit: vi.fn(() => committing.promise),
    });
    const inspector = await openInspector(inventory.items[0]);
    await user.click(within(inspector).getByRole("button", { name: "更新" }));
    await user.click(await screen.findByRole("button", { name: "确认更新" }));

    unmount();
    expect(cancel).not.toHaveBeenCalled();
    committing.resolve(inventory);
    await act(async () => {
      await committing.promise;
    });
    expect(cancel).not.toHaveBeenCalled();
  });

  it("leaves a recovery-owned lifecycle operation untouched after unmount", async () => {
    const user = userEvent.setup();
    const inventory = skillsInventoryFixture();
    const committing = deferred<SkillsInventory>();
    const cancel = vi.fn().mockResolvedValue(undefined);
    const { unmount } = renderWorkspace(inventory, {
      cancel,
      commit: vi.fn(() => committing.promise),
    });
    const inspector = await openInspector(inventory.items[0]);
    await user.click(within(inspector).getByRole("button", { name: "更新" }));
    await user.click(await screen.findByRole("button", { name: "确认更新" }));

    unmount();
    committing.reject({
      code: "recovery_required",
      message: "journal recovery required",
    });
    await act(async () => {
      await committing.promise.catch(() => undefined);
    });
    expect(cancel).not.toHaveBeenCalled();
  });
});
