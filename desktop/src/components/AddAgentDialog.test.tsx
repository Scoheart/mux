import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AddAgentDialog } from "./AddAgentDialog";

const apiMocks = vi.hoisted(() => ({
  addAgent: vi.fn(),
}));

vi.mock("../lib/api", async () => {
  const actual = await vi.importActual<typeof import("../lib/api")>("../lib/api");
  return {
    ...actual,
    addAgent: apiMocks.addAgent,
  };
});

beforeEach(() => {
  apiMocks.addAgent.mockResolvedValue(undefined);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("AddAgentDialog", () => {
  it("starts from Agent identity and switches capability panels with tabs", async () => {
    render(<AddAgentDialog onClose={vi.fn()} onAdded={vi.fn()} />);

    const identity = screen.getByRole("heading", { name: "Agent 身份" });
    const capabilities = screen.getByRole("heading", { name: "MUX 可管理的能力" });
    expect(identity.compareDocumentPosition(capabilities) & Node.DOCUMENT_POSITION_FOLLOWING)
      .toBeTruthy();

    expect(screen.getByLabelText(/Agent 名称/)).toBeVisible();
    expect(screen.getByLabelText(/Agent ID/)).toBeVisible();
    const mcpTab = screen.getByRole("tab", { name: /MCP/ });
    const skillsTab = screen.getByRole("tab", { name: /Skills/ });
    expect(mcpTab).toHaveAttribute("aria-selected", "true");
    expect(skillsTab).toHaveAttribute("aria-selected", "false");
    expect(screen.getByRole("switch", { name: "启用 MCP 能力" })).toBeChecked();
    expect(screen.queryByRole("switch", { name: "启用 Skills 能力" })).not.toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Models：自定义 Agent 暂不可用" })).toBeDisabled();

    await waitFor(() => expect(screen.getByRole("heading", { name: "添加 Agent" })).toHaveFocus());
    mcpTab.focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(skillsTab).toHaveFocus();
    expect(skillsTab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("switch", { name: "启用 Skills 能力" })).not.toBeChecked();
    expect(screen.getByRole("textbox", { name: /^Skills 目录/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: "添加 Agent" })).toBeDisabled();
  });

  it("creates an MCP-capable Agent with identity metadata", async () => {
    const onAdded = vi.fn();
    const onClose = vi.fn();
    render(<AddAgentDialog onClose={onClose} onAdded={onAdded} />);

    await waitFor(() => expect(screen.getByRole("heading", { name: "添加 Agent" })).toHaveFocus());
    await userEvent.type(screen.getByLabelText(/Agent 名称/), "Acme Code");
    await userEvent.type(screen.getByLabelText(/Agent ID/), "acme-code");
    await userEvent.type(screen.getByLabelText(/配置文件/), "~/.acme/mcp.json");
    await userEvent.click(screen.getByRole("button", { name: "添加 Agent" }));

    await waitFor(() => {
      expect(apiMocks.addAgent).toHaveBeenCalledWith("acme-code", {
        global: "~/.acme/mcp.json",
        project: null,
        format: "json",
        key: "mcpServers",
        enabled: true,
        builtin: false,
        name: "Acme Code",
        category: "coding-agent",
        evidence: "custom",
        verified_at: null,
        docs: null,
        skills: null,
      });
    });
    expect(onAdded).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("creates a Skills-only Agent without inventing an MCP writer", async () => {
    render(<AddAgentDialog onClose={vi.fn()} onAdded={vi.fn()} />);

    await waitFor(() => expect(screen.getByRole("heading", { name: "添加 Agent" })).toHaveFocus());
    await userEvent.click(screen.getByRole("tab", { name: /Skills/ }));
    await userEvent.click(screen.getByRole("switch", { name: "启用 Skills 能力" }));
    await userEvent.click(screen.getByRole("tab", { name: /MCP/ }));
    await userEvent.click(screen.getByRole("switch", { name: "启用 MCP 能力" }));
    await userEvent.click(screen.getByRole("tab", { name: /Skills/ }));
    expect(screen.queryByLabelText(/配置文件/)).not.toBeInTheDocument();

    await userEvent.type(screen.getByLabelText(/Agent 名称/), "Acme Skills");
    await userEvent.type(screen.getByLabelText(/Agent ID/), "acme-skills");
    await userEvent.type(
      screen.getByRole("textbox", { name: /^Skills 目录/ }),
      "~/.acme/skills",
    );
    await userEvent.type(
      screen.getByRole("textbox", { name: /^官方文档/ }),
      "https://docs.example.com/skills",
    );
    await userEvent.click(screen.getByRole("button", { name: "添加 Agent" }));

    await waitFor(() => {
      expect(apiMocks.addAgent).toHaveBeenCalledWith("acme-skills", expect.objectContaining({
        global: null,
        format: "",
        key: "",
        name: "Acme Skills",
        docs: "https://docs.example.com/skills",
        skills: {
          target_id: "acme-skills-skills",
          global_dir: "~/.acme/skills",
          aliases: [],
          docs: "https://docs.example.com/skills",
          evidence: "official",
          verified_at: expect.stringMatching(/^\d{4}-\d{2}-\d{2}$/),
          probes: [{ kind: "path", path: "~/.acme/skills" }],
        },
      }));
    });
  });
});
