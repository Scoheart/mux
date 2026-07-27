import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  ResourceInspector,
  ResourceWorkspace,
  WorkspaceSidebar,
} from "./ResourceWorkspace";
import { Modal } from "./ui";

afterEach(cleanup);

beforeEach(() => {
  localStorage.clear();
});

function WorkspaceHarness({ onInspectorClose = vi.fn() }: { onInspectorClose?: () => void }) {
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const closeInspector = () => {
    setInspectorOpen(false);
    onInspectorClose();
  };

  return (
    <ResourceWorkspace
      sidebar={<WorkspaceSidebar title="来源" count={1}>来源内容</WorkspaceSidebar>}
      query=""
      onQueryChange={() => undefined}
      searchPlaceholder="搜索资源"
      toolbarActions={<button type="button">新增</button>}
      inspector={
        inspectorOpen ? (
          <ResourceInspector title="资源 A" avatar={<span>A</span>} onClose={closeInspector}>
            <button type="button" onClick={() => setModalOpen(true)}>打开确认</button>
            {modalOpen && (
              <Modal ariaLabel="确认资源" onClose={() => setModalOpen(false)}>
                <button type="button">确认</button>
              </Modal>
            )}
          </ResourceInspector>
        ) : undefined
      }
      onInspectorClose={closeInspector}
    >
      <button type="button" onClick={() => setInspectorOpen(true)}>打开资源 A</button>
    </ResourceWorkspace>
  );
}

describe("ResourceWorkspace", () => {
  it("labels the resource content as a searchable region", () => {
    render(<WorkspaceHarness />);
    expect(screen.getByRole("region", { name: "搜索资源" })).toBeVisible();
    expect(screen.queryByRole("tablist")).not.toBeInTheDocument();
  });

  it("persists keyboard sidebar resizing within its contract", () => {
    render(<WorkspaceHarness />);
    const separator = screen.getByRole("separator", { name: "调整侧边栏宽度" });
    expect(separator).toHaveAttribute("aria-valuenow", "224");
    fireEvent.keyDown(separator, { key: "End" });
    expect(separator).toHaveAttribute("aria-valuenow", "340");
    expect(localStorage.getItem("mux.resourceWorkspace.sidebarWidth")).toBe("340");
    fireEvent.keyDown(separator, { key: "Home" });
    expect(separator).toHaveAttribute("aria-valuenow", "184");
  });

  it("fills the workspace and skips sidebar persistence when no sidebar is provided", () => {
    const getItem = vi.spyOn(Storage.prototype, "getItem");
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    const { container } = render(
      <ResourceWorkspace
        query=""
        onQueryChange={() => undefined}
        searchPlaceholder="搜索无侧栏资源"
        toolbarActions={<button type="button">新增</button>}
      >
        <div>全部资源</div>
      </ResourceWorkspace>,
    );

    expect(container.querySelector(".mux-workspace")).toHaveAttribute("data-sidebar", "false");
    expect(screen.getByRole("region", { name: "搜索无侧栏资源" })).toBeVisible();
    expect(screen.queryByRole("separator", { name: "调整侧边栏宽度" })).not.toBeInTheDocument();
    expect(getItem).not.toHaveBeenCalledWith("mux.resourceWorkspace.sidebarWidth");
    expect(setItem).not.toHaveBeenCalledWith(
      "mux.resourceWorkspace.sidebarWidth",
      expect.any(String),
    );
    getItem.mockRestore();
    setItem.mockRestore();
  });

  it("opens the Inspector as a centered modal and restores focus after close", async () => {
    render(<WorkspaceHarness />);
    const opener = screen.getByRole("button", { name: "打开资源 A" });
    opener.focus();
    fireEvent.click(opener);

    const panel = document.querySelector(
      '.mux-workspace-scroll[role="region"][aria-label="搜索资源"]',
    );
    expect(panel).not.toBeNull();
    expect(panel).toHaveAttribute("inert");
    expect(panel).toHaveAttribute("aria-hidden", "true");
    const dialog = await screen.findByRole("dialog", { name: "资源详情" });
    expect(dialog).toHaveAttribute("aria-modal", "true");
    await waitFor(() => expect(screen.getByRole("complementary", { name: "资源 A 详情" })).toHaveFocus());

    fireEvent.click(screen.getByRole("button", { name: "关闭详情" }));
    await waitFor(() => expect(opener).toHaveFocus());
  });

  it("lets the topmost modal consume Escape before the Inspector", async () => {
    const onInspectorClose = vi.fn();
    render(<WorkspaceHarness onInspectorClose={onInspectorClose} />);
    fireEvent.click(screen.getByRole("button", { name: "打开资源 A" }));
    expect(await screen.findByRole("dialog", { name: "资源详情" })).toBeVisible();
    await waitFor(() => screen.getByRole("complementary", { name: "资源 A 详情" }));
    fireEvent.click(screen.getByRole("button", { name: "打开确认" }));
    expect(screen.getByRole("dialog", { name: "确认资源" })).toBeVisible();

    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "确认资源" })).not.toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "资源 A 详情" })).toBeVisible();
    expect(onInspectorClose).not.toHaveBeenCalled();

    fireEvent.keyDown(document, { key: "Escape" });
    expect(onInspectorClose).toHaveBeenCalledOnce();
  });
});
