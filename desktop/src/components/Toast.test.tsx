import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { DialogShell } from "./DialogShell";
import { ToastProvider, useToast } from "./Toast";

afterEach(cleanup);

it("keeps an error outside the inert app without stealing dialog focus", async () => {
  function Editor() {
    const toast = useToast();
    return <DialogShell kind="editor" title="Edit provider" onClose={vi.fn()}>
      <button onClick={() => toast.show({ kind: "error", msg: "Save failed" })}>Save</button>
    </DialogShell>;
  }
  const root = document.createElement("div");
  root.id = "root";
  document.body.append(root);
  try {
    render(<ToastProvider><Editor /></ToastProvider>, { container: root });
    await waitFor(() => expect(screen.getByRole("heading")).toHaveFocus());
    const save = screen.getByRole("button", { name: "Save" });
    save.focus();
    fireEvent.click(save);
    const alert = screen.getByRole("alert");
    expect(root).toHaveAttribute("inert");
    expect(root.contains(alert)).toBe(false);
    expect(alert.closest("[inert]")).toBeNull();
    expect(save).toHaveFocus();
    const dismiss = screen.getByRole("button", { name: "关闭错误通知" });
    dismiss.focus();
    fireEvent.keyDown(dismiss, { key: "Tab" });
    expect(screen.getByRole("button", { name: "关闭" })).toHaveFocus();
  } finally { cleanup(); root.remove(); }
});
