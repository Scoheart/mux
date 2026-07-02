#!/usr/bin/env node
import { program } from "commander";
import { migrateIfNeeded } from "./core/settings.js";
import { migrateRegistryToSources } from "./core/sources.js";

// Fold any legacy ~/.mux files into a single settings.json on first run, then
// move manual/discovered registry entries into managed local-source files so the
// CLI and desktop share the same source-based storage.
migrateIfNeeded();
migrateRegistryToSources();

program
  .name("mux")
  .description("MCP Multiplexer — Unified MCP Server configuration manager")
  .version("0.1.0");

program
  .command("clean")
  .description("Clear all MCP configurations from enabled agents")
  .option("--agent <name>", "Only clean a specific agent")
  .action(async (opts) => {
    const { cleanCommand } = await import("./commands/clean.js");
    cleanCommand(opts);
  });

program
  .command("import")
  .description("Scan existing configs and import MCPs to registry")
  .action(async () => {
    const { importCommand } = await import("./commands/import.js");
    importCommand();
  });

program
  .command("list")
  .description("List all MCPs in registry")
  .action(async () => {
    const { listCommand } = await import("./commands/list.js");
    listCommand();
  });

program
  .command("status")
  .description("Show currently active MCPs across all agents")
  .action(async () => {
    const { statusCommand } = await import("./commands/status.js");
    statusCommand();
  });

program
  .command("add <name>")
  .description("Interactively add an MCP to registry")
  .action(async (name) => {
    const { addCommand } = await import("./commands/add.js");
    await addCommand(name);
  });

program
  .command("remove <name>")
  .description("Remove an MCP from registry")
  .action(async (name) => {
    const { removeCommand } = await import("./commands/remove.js");
    removeCommand(name);
  });

program
  .command("apply <names...>")
  .description("Apply MCPs non-interactively")
  .option("--scope <scope>", "Scope: global, project, both", "global")
  .option("--agent <agents>", "Comma-separated agent names", "all")
  .option("--project <dir>", "Project directory for project scope")
  .action(async (names, opts) => {
    const { applyCommand } = await import("./commands/apply.js");
    applyCommand({ names, scope: opts.scope, agent: opts.agent, projectDir: opts.project });
  });

const agentsCmd = program
  .command("agents")
  .description("Manage AI coding agents");

agentsCmd
  .command("list")
  .description("List all agents")
  .action(async () => {
    const { agentsListCommand } = await import("./commands/agents-cmd.js");
    agentsListCommand();
  });

agentsCmd
  .command("enable <name>")
  .description("Enable an agent")
  .action(async (name) => {
    const { agentsEnableCommand } = await import("./commands/agents-cmd.js");
    agentsEnableCommand(name);
  });

agentsCmd
  .command("disable <name>")
  .description("Disable an agent")
  .action(async (name) => {
    const { agentsDisableCommand } = await import("./commands/agents-cmd.js");
    agentsDisableCommand(name);
  });

agentsCmd.action(async () => {
  const { agentsListCommand } = await import("./commands/agents-cmd.js");
  agentsListCommand();
});

program.action(async () => {
  // Clear terminal
  process.stdout.write("\x1B[2J\x1B[H");
  const { render } = await import("ink");
  const React = await import("react");
  const { App } = await import("./tui/app.js");
  const { BreathingBorder } = await import("./tui/breathing-border.js");
  render(React.createElement(BreathingBorder, null, React.createElement(App)));
});

program.parse();
