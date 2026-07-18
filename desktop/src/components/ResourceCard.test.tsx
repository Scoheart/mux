import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ResourceCard } from "./ResourceCard";

afterEach(cleanup);

describe("ResourceCard", () => {
  it("renders the shared slot order and exposes selection", () => {
    const { container } = render(
      <ResourceCard
        identity={<span>Identity</span>}
        configuration={<span>Configuration</span>}
        state={<span>State</span>}
        impact={<span>Impact</span>}
        selected
        attention="shadowed"
        ariaLabel="打开资源"
        onOpen={() => undefined}
      />,
    );

    const card = screen.getByRole("button", { name: "打开资源" });
    expect(card).toHaveAttribute("aria-pressed", "true");
    expect(card).toHaveAttribute("data-attention", "shadowed");
    expect(
      Array.from(container.querySelectorAll("[data-resource-slot]")).map((node) =>
        node.getAttribute("data-resource-slot"),
      ),
    ).toEqual(["identity", "configuration", "state", "impact"]);
  });

  it("opens with pointer, Enter, or Space", () => {
    const onOpen = vi.fn();
    render(
      <ResourceCard identity="Resource" ariaLabel="打开资源" onOpen={onOpen} />,
    );

    const card = screen.getByRole("button", { name: "打开资源" });
    fireEvent.click(card);
    fireEvent.keyDown(card, { key: "Enter" });
    fireEvent.keyDown(card, { key: " " });
    expect(onOpen).toHaveBeenCalledTimes(3);
  });

  it("does not activate from a nested control", () => {
    const onOpen = vi.fn();
    render(
      <ResourceCard
        identity={<button type="button">辅助操作</button>}
        ariaLabel="打开资源"
        onOpen={onOpen}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "辅助操作" }));
    fireEvent.keyDown(screen.getByRole("button", { name: "辅助操作" }), { key: "Enter" });
    expect(onOpen).not.toHaveBeenCalled();
  });
});
