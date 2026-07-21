import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SkillsState } from "../hooks/useSkillsState";
import * as api from "../lib/api";
import type { OperationPlan, SkillInventoryItem, SkillSourceResolution, SkillsInventory } from "../lib/types";
import {
  resolutionFixture,
  sharedTargetPlanFixture,
  highRiskPlan,
  skillsInventoryFixture,
  skillsStateFixture,
} from "../test/skillsFixtures";
import { SkillInstallDialog } from "./SkillInstallDialog";
import { SkillsView } from "./SkillsView";
import { ToastProvider } from "./Toast";

const dialogCss = await readFile(resolve(process.cwd(), "src/index.css"), "utf8");

vi.mock("../lib/api", async () => {
  const actual = await vi.importActual<typeof import("../lib/api")>("../lib/api");
  return {
    ...actual,
    getSkillDetail: vi.fn(),
    resolveGithubSkillSource: vi.fn(),
    resolveLocalSkillSourceDialog: vi.fn(),
    resolveArchiveSkillSourceDialog: vi.fn(),
    planSkillAssetInstall: vi.fn(),
    planSkillAssetImport: vi.fn(),
    planSkillUpdate: vi.fn(),
    planSkillRemove: vi.fn(),
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

const safeInstallPlan = (): OperationPlan => ({
  ...sharedTargetPlanFixture(),
  targets: [],
  warnings: [],
});

function renderInstall(overrides: {
  commit?: SkillsState["commit"];
  cancel?: SkillsState["cancel"];
  onClose?: () => void;
  onCommitted?: (inventory: SkillsInventory) => void;
  onRecoveryRequired?: (message: string) => void;
} = {}) {
  const state = skillsStateFixture();
  const props = {
    commit: overrides.commit ?? state.commit,
    cancel: overrides.cancel ?? state.cancel,
    onClose: overrides.onClose ?? vi.fn(),
    onCommitted: overrides.onCommitted ?? vi.fn(),
    onRecoveryRequired: overrides.onRecoveryRequired ?? vi.fn(),
  };
  const rendered = render(
    <ToastProvider><SkillInstallDialog {...props} /></ToastProvider>,
  );
  return { ...rendered, props };
}

async function resolveGithub(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText("仓库地址"), "  acme/skills  ");
  await user.click(screen.getByRole("button", { name: "查找" }));
  await screen.findByRole("checkbox", { name: "review-changes" });
}

beforeEach(() => {
  vi.mocked(api.resolveGithubSkillSource).mockResolvedValue(resolutionFixture());
  vi.mocked(api.resolveLocalSkillSourceDialog).mockResolvedValue(null);
  vi.mocked(api.resolveArchiveSkillSourceDialog).mockResolvedValue(null);
  vi.mocked(api.planSkillAssetInstall).mockResolvedValue(safeInstallPlan());
  vi.mocked(api.planSkillAssetImport).mockResolvedValue(planAs("import"));
  vi.mocked(api.planSkillUpdate).mockResolvedValue(planAs("update"));
  vi.mocked(api.planSkillRemove).mockResolvedValue(planAs("remove"));
  vi.mocked(api.planSkillRepair).mockResolvedValue(planAs("repair"));
  vi.mocked(api.getSkillDetail).mockImplementation(async (identity) => ({
    item: skillsInventoryFixture().items.find((item) => item.identity === identity) ?? skillsInventoryFixture().items[0],
    files: [],
    skill_md: "---\nname: fixture\n---\n",
    skill_md_truncated: false,
  }));
});

afterEach(cleanup);

describe("SkillInstallDialog central asset intake", () => {
  it("presents GitHub, folder, and archive as three peer source entries", () => {
    renderInstall();
    expect(screen.getByRole("heading", { name: "GitHub" })).toBeVisible();
    expect(screen.getByRole("button", { name: "选择本地文件夹" })).toBeVisible();
    expect(screen.getByRole("button", { name: "选择 Skill 压缩包" })).toBeVisible();
    expect(screen.queryByText("或")).not.toBeInTheDocument();
    expect(document.querySelector(".mux-skill-source-divider")).not.toBeInTheDocument();
    expect(document.querySelector(".mux-skill-local-sources")).not.toBeInTheDocument();
  });

  it("selects only central candidates and never exposes Agent assignment", async () => {
    const user = userEvent.setup();
    vi.mocked(api.resolveGithubSkillSource).mockResolvedValueOnce(twoCandidateResolution());
    renderInstall();
    await resolveGithub(user);

    expect(api.resolveGithubSkillSource).toHaveBeenCalledWith("acme/skills");
    expect(screen.getByRole("checkbox", { name: "review-changes" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "release-notes" })).toBeChecked();
    expect(screen.queryByText("目标 Agent")).not.toBeInTheDocument();

    await user.click(screen.getByRole("checkbox", { name: "release-notes" }));
    await user.click(screen.getByRole("button", { name: "下载 Skill" }));
    expect(api.planSkillAssetInstall).toHaveBeenCalledWith({
      resolution_id: "resolve-fixture",
      skill_names: ["review-changes"],
      replace_conflicts: false,
    });
    expect(JSON.stringify(vi.mocked(api.planSkillAssetInstall).mock.calls[0][0])).not.toContain("agent");
  });

  it("uses the native local picker and keeps the source step on cancel", async () => {
    renderInstall();
    await userEvent.click(screen.getByRole("button", { name: "选择本地文件夹" }));
    expect(api.resolveLocalSkillSourceDialog).toHaveBeenCalledOnce();
    expect(screen.getByRole("heading", { name: "添加 Skill" })).toBeVisible();
  });

  it("uses the native archive picker and keeps the source step on cancel", async () => {
    renderInstall();
    await userEvent.click(screen.getByRole("button", { name: "选择 Skill 压缩包" }));
    expect(api.resolveArchiveSkillSourceDialog).toHaveBeenCalledOnce();
    expect(screen.getByRole("heading", { name: "添加 Skill" })).toBeVisible();
  });

  it("imports an archive directly without opening an audit dialog", async () => {
    const resolution: SkillSourceResolution = {
      ...resolutionFixture(),
      source: {
        kind: "archive",
        path: "~/Downloads/skills.zip",
        subpath: "review-changes",
      },
      resolved_revision: null,
    };
    const plan = safeInstallPlan();
    const commit = vi.fn().mockResolvedValue(skillsInventoryFixture());
    vi.mocked(api.resolveArchiveSkillSourceDialog).mockResolvedValueOnce(resolution);
    vi.mocked(api.planSkillAssetInstall).mockResolvedValueOnce(plan);
    renderInstall({ commit });

    await userEvent.click(screen.getByRole("button", { name: "选择 Skill 压缩包" }));
    await screen.findByRole("checkbox", { name: "review-changes" });
    await userEvent.click(screen.getByRole("button", { name: "导入 Skill" }));

    await waitFor(() => expect(commit).toHaveBeenCalledWith(plan, null));
    expect(screen.queryByRole("dialog", { name: "审阅 Skill 操作" })).not.toBeInTheDocument();
  });

  it("asks to back up only after a same-name conflict", async () => {
    const user = userEvent.setup();
    const cancel = vi.fn().mockResolvedValue(undefined);
    vi.mocked(api.planSkillAssetInstall)
      .mockRejectedValueOnce({ code: "conflict", message: "central Skill content already exists" })
      .mockResolvedValueOnce(safeInstallPlan());
    renderInstall({ cancel });
    await resolveGithub(user);
    await user.click(screen.getByRole("button", { name: "下载 Skill" }));
    expect(await screen.findByText("发现冲突")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "备份并下载" }));
    expect(api.planSkillAssetInstall).toHaveBeenCalledTimes(2);
    expect(vi.mocked(api.planSkillAssetInstall).mock.calls[0][0]).toEqual({
      resolution_id: "resolve-fixture",
      skill_names: ["review-changes"],
      replace_conflicts: false,
    });
    expect(vi.mocked(api.planSkillAssetInstall).mock.calls[1][0]).toEqual({
      resolution_id: "resolve-fixture",
      skill_names: ["review-changes"],
      replace_conflicts: true,
    });
    expect(cancel).not.toHaveBeenCalled();
  });

  it("coalesces close cleanup and cancels the staged resolution once", async () => {
    const user = userEvent.setup();
    const cancellation = deferred<void>();
    const cancel = vi.fn(() => cancellation.promise);
    const onClose = vi.fn();
    renderInstall({ cancel, onClose });
    await resolveGithub(user);
    const close = screen.getByRole("button", { name: "关闭" });
    fireEvent.click(close);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(cancel).toHaveBeenCalledOnce();
    cancellation.resolve();
    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  });

  it("does not cancel content after a successful commit", async () => {
    const user = userEvent.setup();
    const committing = deferred<SkillsInventory>();
    const commit = vi.fn(() => committing.promise);
    const cancel = vi.fn().mockResolvedValue(undefined);
    const onCommitted = vi.fn();
    renderInstall({ commit, cancel, onCommitted });
    await resolveGithub(user);
    await user.click(screen.getByRole("button", { name: "下载 Skill" }));
    expect(commit).toHaveBeenCalledOnce();
    expect(cancel).not.toHaveBeenCalled();
    const inventory = skillsInventoryFixture();
    committing.resolve(inventory);
    await waitFor(() => expect(onCommitted).toHaveBeenCalledWith(inventory));
    expect(cancel).not.toHaveBeenCalled();
  });

  it("leaves recovery-owned staging untouched after unmount", async () => {
    const user = userEvent.setup();
    const committing = deferred<SkillsInventory>();
    const cancel = vi.fn().mockResolvedValue(undefined);
    const { unmount } = renderInstall({ commit: vi.fn(() => committing.promise), cancel });
    await resolveGithub(user);
    await user.click(screen.getByRole("button", { name: "下载 Skill" }));
    unmount();
    committing.reject({ code: "recovery_required", message: "recovery required" });
    await act(async () => { await committing.promise.catch(() => undefined); });
    expect(cancel).not.toHaveBeenCalled();
  });

  it("downloads high-risk content directly with the plan-bound findings hash", async () => {
    const user = userEvent.setup();
    const plan = highRiskPlan("high-risk");
    const commit = vi.fn().mockResolvedValue(skillsInventoryFixture());
    vi.mocked(api.planSkillAssetInstall).mockResolvedValueOnce(plan);
    renderInstall({ commit });
    await resolveGithub(user);

    await user.click(screen.getByRole("button", { name: "下载 Skill" }));

    await waitFor(() => expect(commit).toHaveBeenCalledWith(plan, "high-risk"));
    expect(screen.queryByRole("dialog", { name: "审阅 Skill 操作" })).not.toBeInTheDocument();
  });
});

describe("Skills central lifecycle orchestration", () => {
  const renderWorkspace = (inventory: SkillsInventory, overrides: Partial<SkillsState> = {}) => {
    const state = { ...skillsStateFixture(), inventory, ...overrides };
    return render(<ToastProvider><SkillsView state={state} /></ToastProvider>);
  };

  async function openInspector(item: SkillInventoryItem) {
    await userEvent.click(screen.getByRole("button", { name: new RegExp(item.name) }));
    return screen.findByLabelText(`${item.name} 详情`);
  }

  it("opens central intake only from the top-level toolbar", async () => {
    renderWorkspace(skillsInventoryFixture());
    await userEvent.click(screen.getByRole("button", { name: "添加 Skill" }));
    expect(screen.getByRole("dialog", { name: "添加 Skill" })).toBeVisible();
    expect(screen.queryByText("目标 Agent")).not.toBeInTheDocument();
  });

  it("imports an external copy without implicit Agent ids", async () => {
    const inventory = skillsInventoryFixture();
    const external: SkillInventoryItem = {
      ...inventory.items[1],
      identity: "target:agents-user:external-copy",
      name: "external-copy",
      states: ["external"],
      location: { kind: "agent_target", target_id: "agents-user", global_dir: "~/.agents/skills" },
      source: null,
      affected_agent_ids: ["codex", "cursor", "gemini"],
    };
    inventory.items = [external];
    const plan = planAs("import");
    const commit = vi.fn().mockResolvedValue(skillsInventoryFixture());
    vi.mocked(api.planSkillAssetImport).mockResolvedValueOnce(plan);
    renderWorkspace(inventory, { commit });
    const inspector = await openInspector(external);
    await userEvent.click(within(inspector).getByRole("button", { name: "导入" }));
    expect(api.planSkillAssetImport).toHaveBeenCalledWith({
      identity: external.identity,
      replace_conflicts: false,
    });
    expect(vi.mocked(api.planSkillAssetImport).mock.calls[0][0]).not.toHaveProperty("agent_ids");
    await waitFor(() => expect(commit).toHaveBeenCalledWith(plan, null));
    expect(screen.queryByRole("dialog", { name: "审阅 Skill 操作" })).not.toBeInTheDocument();
  });

  it("has no direct assignment switches in the Skill inspector", async () => {
    const inventory = skillsInventoryFixture();
    renderWorkspace(inventory);
    const inspector = await openInspector(inventory.items[0]);
    expect(within(inspector).queryByRole("switch")).not.toBeInTheDocument();
  });
});

describe("Skill dialog layout contract", () => {
  it("keeps dialog bodies bounded and scrollable", () => {
    expect(dialogCss).toContain(".mux-skill-dialog-body");
    expect(dialogCss).toContain(".mux-skill-review-body");
    expect(dialogCss).toContain("overflow-y: auto");
    expect(dialogCss).toContain("@media (max-width: 920px)");
    expect(dialogCss).toContain("@media (max-height: 640px)");
  });
});
