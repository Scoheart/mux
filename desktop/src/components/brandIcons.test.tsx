import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { AgentGlyph } from "./brandIcons";

afterEach(cleanup);

function surfaceFor(container: HTMLElement): string | null {
  return container
    .querySelector<HTMLElement>("[data-agent-surface]")
    ?.getAttribute("data-agent-surface") ?? null;
}

it("distinguishes Claude and Qoder variants that share a logo", () => {
  const cases = [
    ["claude-code", "Claude Code", "cli"],
    ["claude-desktop", "Claude Desktop", "desktop"],
    ["qoder-cli", "Qoder CLI", "cli"],
    ["qoder", "Qoder IDE", "ide"],
    ["qoder-desktop", "Qoder Desktop", "desktop"],
  ] as const;

  for (const [id, name, surface] of cases) {
    const view = render(<AgentGlyph id={id} name={name} size={30} />);
    expect(surfaceFor(view.container), id).toBe(surface);
    expect(view.container.querySelector("[data-agent-surface]")).toHaveAttribute(
      "aria-hidden",
      "true",
    );
    expect(view.getByAltText(name)).toBeVisible();
    view.unmount();
  }
});

it("keeps unique, custom, and fallback Agent icons unbadged", () => {
  for (const [id, name] of [
    ["codex", "Codex"],
    ["cursor", "Cursor"],
    ["my-custom-agent", "My Custom Agent"],
  ]) {
    const view = render(<AgentGlyph id={id} name={name} size={30} />);
    expect(surfaceFor(view.container), id).toBeNull();
    view.unmount();
  }
});

it("uses the compact, regular, and large badge size tiers", () => {
  const cases = [
    [20, "10px"],
    [24, "10px"],
    [30, "12px"],
    [32, "12px"],
    [42, "14px"],
    [44, "14px"],
  ] as const;

  for (const [size, expected] of cases) {
    const view = render(
      <AgentGlyph id="claude-code" name="Claude Code" size={size} />,
    );
    expect(
      view.container.querySelector<HTMLElement>("[data-agent-surface]"),
    ).toHaveStyle({ width: expected, height: expected });
    view.unmount();
  }
});
