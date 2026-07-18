import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  ResourceInspector,
  ResourceTabs,
  ResourceWorkspace,
  WorkspaceSidebar,
} from "./ResourceWorkspace";
import { Modal } from "./ui";

afterEach(cleanup);

beforeEach(() => {
  localStorage.clear();
});

function WorkspaceHarness({ onInspectorClose = vi.fn() }: { onInspectorClose?: () => void }) {
  const [tab, setTab] = useState<"all" | "used" | "unused">("all");
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
      filters={
        <ResourceTabs
          label="资源状态"
          value={tab}
          onChange={setTab}
          options={[
            { value: "all", label: "全部", count: 3 },
            { value: "used", label: "使用中", count: 2 },
            { value: "unused", label: "未使用", count: 1 },
          ]}
        />
      }
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
  it("links tabs to the panel and supports roving focus", () => {
    render(<WorkspaceHarness />);
    const all = screen.getByRole("tab", { name: /全部/ });
    const panel = screen.getByRole("tabpanel", { name: /全部/ });
    expect(all).toHaveAttribute("aria-controls", panel.id);
    expect(panel).toHaveAttribute("aria-labelledby", all.id);

    fireEvent.keyDown(all, { key: "End" });
    expect(screen.getByRole("tab", { name: /未使用/ })).toHaveFocus();
    expect(screen.getByRole("tab", { name: /未使用/ })).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(screen.getByRole("tab", { name: /未使用/ }), { key: "Home" });
    expect(all).toHaveFocus();
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

  it("makes the resource panel inert and restores focus after Inspector close", async () => {
    render(<WorkspaceHarness />);
    const opener = screen.getByRole("button", { name: "打开资源 A" });
    opener.focus();
    fireEvent.click(opener);

    const panel = screen.getByRole("tabpanel", { hidden: true });
    expect(panel).toHaveAttribute("inert");
    expect(panel).toHaveAttribute("aria-hidden", "true");
    await waitFor(() => {
      expect(screen.getByRole("complementary", { name: "资源 A 详情" })).toHaveFocus();
    });

    fireEvent.click(screen.getAllByRole("button", { name: "关闭详情" })[0]);
    await waitFor(() => expect(opener).toHaveFocus());
  });

  it("lets the topmost modal consume Escape before the Inspector", async () => {
    const onInspectorClose = vi.fn();
    render(<WorkspaceHarness onInspectorClose={onInspectorClose} />);
    fireEvent.click(screen.getByRole("button", { name: "打开资源 A" }));
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
