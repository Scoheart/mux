import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { ResourceCard } from "./ResourceCard";

afterEach(cleanup);

it("keeps representative MCP, Model, and Skill cards on one slot contract", () => {
  for (const domain of ["MCP", "Model", "Skill"]) {
    render(
      <ResourceCard
        ariaLabel={`打开 ${domain}`}
        onOpen={() => undefined}
        identity={`${domain} identity`}
        configuration={`${domain} configuration`}
        state={`${domain} state`}
        impact={`${domain} impact`}
      />,
    );
  }

  for (const card of screen.getAllByRole("button")) {
    expect(
      Array.from(card.querySelectorAll("[data-resource-slot]")).map((node) =>
        node.getAttribute("data-resource-slot"),
      ),
    ).toEqual(["identity", "configuration", "state", "impact"]);
  }
});
