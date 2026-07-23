import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { Avatar } from "./ui";

afterEach(cleanup);

it("renders the resource initial with a stable per-asset color", () => {
  const { container, rerender } = render(
    <Avatar seed="@context7" kind="mcp" size={34} />,
  );
  const first = container.firstElementChild as HTMLElement;
  const background = first.style.background;
  expect(first).toHaveTextContent("C");
  expect(first).toHaveAttribute("aria-hidden", "true");
  expect(first).toHaveAttribute("data-kind", "mcp");

  rerender(<Avatar seed="@context7" kind="mcp" size={34} />);
  expect((container.firstElementChild as HTMLElement).style.background).toBe(background);

  rerender(<Avatar seed="OpenRouter" kind="model" size={34} />);
  expect(container.firstElementChild).toHaveTextContent("O");
  expect((container.firstElementChild as HTMLElement).style.background).not.toBe(background);
});

it("prefers a supplied icon over the monogram", () => {
  const { container } = render(
    <Avatar seed="OpenAI" kind="model" icon={<svg data-provider-icon="openai" />} />,
  );
  expect(container.querySelector('[data-provider-icon="openai"]')).not.toBeNull();
  expect(container.firstElementChild).not.toHaveTextContent("O");
});
