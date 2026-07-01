import { useState, useMemo, useRef, useEffect } from "react";
import { Box, Text, useInput, useStdout } from "ink";
import TextInput from "ink-text-input";
import { createMcpSearcher } from "../utils/fuzzy.js";
import { Logo } from "./logo.js";
import type { RegistryEntry, AgentsConfig } from "../types.js";

// Check if a character is CJK (occupies 2 terminal columns)
function isCJK(char: string): boolean {
  const code = char.charCodeAt(0);
  return (
    (code >= 0x4E00 && code <= 0x9FFF) ||   // CJK Unified
    (code >= 0x3400 && code <= 0x4DBF) ||   // CJK Extension A
    (code >= 0xF900 && code <= 0xFAFF) ||   // CJK Compatibility
    (code >= 0xFF00 && code <= 0xFF60) ||   // Fullwidth Forms
    (code >= 0x3000 && code <= 0x303F)      // CJK Symbols
  );
}

function truncateByDisplayWidth(str: string, maxWidth: number): string {
  let width = 0;
  for (let i = 0; i < str.length; i++) {
    const charWidth = isCJK(str[i]) ? 2 : 1;
    if (width + charWidth > maxWidth) {
      return str.slice(0, i) + "…";
    }
    width += charWidth;
  }
  return str;
}

export type ScopeTab = "project" | "global" | "registry" | "agents";

export interface McpSelection {
  name: string;
  projectAgents: string[];
  globalAgents: string[];
}

interface Props {
  entries: RegistryEntry[];
  selections: McpSelection[];
  enabledAgents: string[];
  activeTab: ScopeTab;
  cursor: number;
  initialFocus?: "tabs" | "search" | "list";
  message?: string;
  agentsConfig: AgentsConfig;
  onCursorChange: (cursor: number) => void;
  onTabChange: (tab: ScopeTab) => void;
  onToggle: (name: string, tab: ScopeTab) => void;
  onOpenConfig: (mcpName: string, tab: ScopeTab) => void;
  onAdd: (tab: ScopeTab) => void;
  onApply: () => void;
  onQuit: () => void;
  onRegistryApply: (name: string) => void;
  onRegistryEdit: (name: string) => void;
  onRegistryAdd: () => void;
  onRegistryDelete: (name: string) => void;
  onAgentToggle: (name: string) => void;
}

const LOGO_LINES = 4;          // 2 text lines + 1 marginBottom + 1 buffer
const NON_LIST_LINES = 11;    // tab(1)+margin(1) + search(3)+margin(1) + scrollUp(1) + scrollDown(1) + emptyGap(1) + footer(1) + buffer(1)
const MIN_LIST_ROWS = 3;
const DEFAULT_MAX_VISIBLE = 15;
const TABS: ScopeTab[] = ["project", "global", "registry", "agents"];

export function McpList({ entries, selections, enabledAgents, activeTab, cursor, initialFocus, message, agentsConfig, onCursorChange, onTabChange, onToggle, onOpenConfig, onAdd, onApply, onQuit, onRegistryApply, onRegistryEdit, onRegistryAdd, onRegistryDelete, onAgentToggle }: Props) {
  const [query, setQuery] = useState("");
  const [focusArea, setFocusArea] = useState<"search" | "tabs" | "list">(initialFocus ?? "tabs");
  const { stdout } = useStdout();

  // Adaptive layout: always show logo, reduce list rows if terminal is short
  const terminalRows = stdout?.rows ?? 40;
  const availableForList = terminalRows - NON_LIST_LINES - LOGO_LINES;
  const maxVisible = Math.max(MIN_LIST_ROWS, Math.min(DEFAULT_MAX_VISIBLE, availableForList));

  const setCursor = (updater: number | ((c: number) => number)) => {
    if (typeof updater === "function") {
      onCursorChange(updater(cursor));
    } else {
      onCursorChange(updater);
    }
  };

  const searcher = useMemo(() => createMcpSearcher(entries), [entries]);

  // Filter entries based on active tab
  const filteredByTab = useMemo(() => {
    const all = searcher(query);
    if (activeTab === "registry") return all;
    if (activeTab === "agents") return [];
    return all.filter((entry) => {
      const sel = selections.find((s) => s.name === entry.name);
      if (!sel) return false;
      const targets = activeTab === "project" ? sel.projectAgents : sel.globalAgents;
      return targets.length > 0;
    });
  }, [searcher, query, activeTab, selections]);

  // Clamp cursor when filtered list changes (e.g. tab switch, search)
  useEffect(() => {
    if (cursor >= filteredByTab.length && filteredByTab.length > 0) {
      onCursorChange(filteredByTab.length - 1);
    }
  }, [filteredByTab.length]);

  const scrollOffset = useRef(0);
  if (cursor < scrollOffset.current) {
    scrollOffset.current = cursor;
  } else if (cursor >= scrollOffset.current + maxVisible) {
    scrollOffset.current = cursor - maxVisible + 1;
  }

  const visibleItems = filteredByTab.slice(scrollOffset.current, scrollOffset.current + maxVisible);
  const hiddenBelow = Math.max(0, filteredByTab.length - scrollOffset.current - maxVisible);
  const hiddenAbove = scrollOffset.current;

  useInput((input, key) => {
    // Ctrl+S to apply
    if (input === "s" && key.ctrl) {
      onApply();
      return;
    }

    // Ctrl+A to add MCP (works from any focus area, only in project/global)
    if (input === "a" && key.ctrl && activeTab !== "registry") {
      onAdd(activeTab);
      return;
    }

    // Ctrl+A in registry tab — add new MCP to registry
    if (input === "a" && key.ctrl && activeTab === "registry") {
      onRegistryAdd();
      return;
    }

    // Tab key to focus search input
    if (key.tab) {
      setFocusArea("search");
      return;
    }

    // Number keys 1/2/3 to jump to tab directly (any focus)
    if (input === "1") {
      onTabChange("project");
      onCursorChange(0);
      scrollOffset.current = 0;
      return;
    }
    if (input === "2") {
      onTabChange("global");
      onCursorChange(0);
      scrollOffset.current = 0;
      return;
    }
    if (input === "3") {
      onTabChange("registry");
      onCursorChange(0);
      scrollOffset.current = 0;
      return;
    }
    if (input === "4") {
      onTabChange("agents");
      onCursorChange(0);
      scrollOffset.current = 0;
      return;
    }

    // Ctrl+C or q to quit
    if ((input === "c" && key.ctrl) || (input === "q" && focusArea !== "search")) {
      onQuit();
      return;
    }

    // --- Focus: tabs ---
    if (focusArea === "tabs") {
      if (key.downArrow) {
        setFocusArea("search");
        return;
      }
      if (key.leftArrow) {
        const currentIndex = TABS.indexOf(activeTab);
        const prevTab = TABS[(currentIndex - 1 + TABS.length) % TABS.length];
        onTabChange(prevTab);
        onCursorChange(0);
        scrollOffset.current = 0;
        return;
      }
      if (key.rightArrow) {
        const currentIndex = TABS.indexOf(activeTab);
        const nextTab = TABS[(currentIndex + 1) % TABS.length];
        onTabChange(nextTab);
        onCursorChange(0);
        scrollOffset.current = 0;
        return;
      }
      return;
    }

    // --- Focus: search ---
    if (focusArea === "search" && key.upArrow) {
      setFocusArea("tabs");
      return;
    }
    if (focusArea === "search" && key.downArrow) {
      setFocusArea("list");
      return;
    }

    // --- Focus: list ---
    if (focusArea === "list") {
      // "/" to jump to search from anywhere in the list
      if (input === "/") {
        setFocusArea("search");
        return;
      }

      if (key.upArrow) {
        if (cursor === 0) {
          setFocusArea("search");
        } else {
          setCursor((c) => c - 1);
        }
        return;
      }
      if (key.downArrow) {
        const maxIdx = activeTab === "agents"
          ? Object.keys(agentsConfig.agents).length - 1
          : filteredByTab.length - 1;
        setCursor((c) => Math.min(maxIdx, c + 1));
        return;
      }

      // Space in agents tab — toggle agent enabled/disabled
      if (input === " " && activeTab === "agents") {
        const agentNames = Object.keys(agentsConfig.agents);
        const name = agentNames[cursor];
        if (name) onAgentToggle(name);
        return;
      }

      // "d" to remove from current scope (project/global only), with toggle
      if (input === "d" && activeTab !== "registry" && activeTab !== "agents") {
        const item = filteredByTab[cursor];
        if (item) onToggle(item.name, activeTab);
        return;
      }

      // "d" in registry tab — delete MCP from registry
      if (input === "d" && activeTab === "registry") {
        const item = filteredByTab[cursor];
        if (item) onRegistryDelete(item.name);
        return;
      }

      // "e" in registry tab — edit MCP
      if (input === "e" && activeTab === "registry") {
        const item = filteredByTab[cursor];
        if (item) onRegistryEdit(item.name);
        return;
      }

      // Enter to configure targets (project/global) or apply from registry
      if (key.return) {
        const item = filteredByTab[cursor];
        if (item && activeTab === "registry") {
          onRegistryApply(item.name);
        } else if (item) {
          onOpenConfig(item.name, activeTab);
        }
        return;
      }
    }
  });

  const handleQueryChange = (value: string) => {
    setQuery(value);
    setCursor(0);
    scrollOffset.current = 0;
  };

  const projectCount = selections.filter((s) => s.projectAgents.length > 0).length;
  const globalCount = selections.filter((s) => s.globalAgents.length > 0).length;
  const agentEntries = Object.entries(agentsConfig.agents);
  const enabledAgentCount = agentEntries.filter(([, def]) => def.enabled).length;
  const nameWidth = Math.max(...entries.map((e) => e.name.length), 10) + 4;

  return (
    <Box flexDirection="column" marginTop={1} height={terminalRows - 1}>
      {/* Logo */}
      <Box marginBottom={2}>
        <Logo />
      </Box>

      {/* Tab bar */}
      <Box marginBottom={1} gap={3}>
        {focusArea === "tabs" && <Text color="cyan">›</Text>}
        {activeTab === "project" ? (
          <Text bold inverse> Project ({projectCount}) </Text>
        ) : (
          <Text color="gray">Project ({projectCount})</Text>
        )}
        {activeTab === "global" ? (
          <Text bold inverse> Global ({globalCount}) </Text>
        ) : (
          <Text color="gray">Global ({globalCount})</Text>
        )}
        {activeTab === "registry" ? (
          <Text bold inverse> Registry ({entries.length}) </Text>
        ) : (
          <Text color="gray">Registry ({entries.length})</Text>
        )}
        {activeTab === "agents" ? (
          <Text bold inverse> Agents ({enabledAgentCount}/{agentEntries.length}) </Text>
        ) : (
          <Text color="gray">Agents ({enabledAgentCount}/{agentEntries.length})</Text>
        )}
      </Box>

      {/* Search input with border */}
      <Box
        borderStyle="round"
        borderColor={focusArea === "search" ? "cyan" : "gray"}
        paddingX={1}
        marginBottom={1}
      >
        <Text color="gray">🔍 </Text>
        {focusArea === "search" ? (
          <TextInput
            value={query}
            onChange={handleQueryChange}
            placeholder="Search MCPs..."
          />
        ) : (
          <Text dimColor>{query || "Search MCPs..."}</Text>
        )}
      </Box>

      {/* Column headers */}
      {activeTab === "agents" ? (
        <Box>
          <Box width={3}><Text>{" "}</Text></Box>
          <Box width={5}><Text dimColor bold>ON</Text></Box>
          <Box width={16}><Text dimColor bold>AGENT</Text></Box>
          <Box width={7}><Text dimColor bold>FMT</Text></Box>
          <Box width={14}><Text dimColor bold>KEY</Text></Box>
          <Text dimColor bold>GLOBAL PATH</Text>
        </Box>
      ) : activeTab === "registry" ? (
        <Box>
          <Box width={3}><Text>{" "}</Text></Box>
          <Box width={nameWidth}><Text dimColor bold>NAME</Text></Box>
          <Box width={7}><Text dimColor bold>TYPE</Text></Box>
          <Box width={9}><Text dimColor bold>KIND</Text></Box>
          <Box width={11}><Text dimColor bold>SOURCE</Text></Box>
          <Text dimColor bold>DESCRIPTION</Text>
        </Box>
      ) : (
        <Box>
          <Box width={3}><Text>{" "}</Text></Box>
          <Box width={nameWidth}><Text dimColor bold>NAME</Text></Box>
          <Text dimColor bold>AGENTS</Text>
        </Box>
      )}

      {/* Scroll indicator above (always reserve space) */}
      <Box>
        <Text dimColor>{hiddenAbove > 0 ? `  ↑ ${hiddenAbove} more above` : " "}</Text>
      </Box>

      {/* Agents tab — dedicated list */}
      {activeTab === "agents" && agentEntries.map(([agentName, def], i) => {
        const isCursor = i === cursor && focusArea === "list";
        const statusIcon = def.enabled ? "✓" : "✗";
        const statusColor = def.enabled ? "green" : "red";
        const globalPath = def.global ?? "—";
        return (
          <Box key={agentName}>
            <Box width={3}>
              <Text color={isCursor ? "blue" : undefined}>
                {isCursor ? " ›" : "  "}
              </Text>
            </Box>
            <Box width={5}>
              <Text color={statusColor} bold>{statusIcon}</Text>
            </Box>
            <Box width={16}>
              <Text color={isCursor ? "blue" : "white"} bold={isCursor}>
                {agentName}
              </Text>
            </Box>
            <Box width={7}><Text color="gray">{def.format}</Text></Box>
            <Box width={14}><Text color="gray">{def.key}</Text></Box>
            <Text dimColor wrap="truncate">{globalPath}</Text>
          </Box>
        );
      })}

      {/* MCP List - only render actual items, no empty padding */}
      {activeTab !== "agents" && visibleItems.map((entry, i) => {
        const actualIndex = i + scrollOffset.current;
        const sel = selections.find((s) => s.name === entry.name);
        const isCursor = actualIndex === cursor && focusArea === "list";

        let statusText: string;
        let statusColor: string;

        if (activeTab === "registry") {
          statusText = "";
          statusColor = "gray";
        } else {
          const targets = sel
            ? (activeTab === "project" ? sel.projectAgents : sel.globalAgents)
            : [];
          statusText = targets.join(", ");
          statusColor = "blue";
        }

        if (activeTab === "registry") {
          const regEntry = entries.find((e) => e.name === entry.name);
          const typePart = regEntry?.config?.stdio ? "stdio" : regEntry?.config?.http ? "http" : "—";
          const kind = regEntry?.tags?.includes("builtin") ? "built-in" : "custom";
          const source = regEntry?.tags?.includes("official")
            ? "official"
            : regEntry?.tags?.includes("community")
              ? "community"
              : "other";
          const rawDesc = regEntry?.description ?? "";
          // Truncate description accounting for CJK double-width characters
          const termCols = stdout?.columns ?? 80;
          const usedCols = 3 + nameWidth + 7 + 9 + 11;
          const descMaxCols = Math.max(0, termCols - usedCols - 2);
          const desc = truncateByDisplayWidth(rawDesc, descMaxCols);
          return (
            <Box key={entry.name}>
              <Box width={3}>
                <Text color={isCursor ? "blue" : undefined}>
                  {isCursor ? " ›" : "  "}
                </Text>
              </Box>
              <Box width={nameWidth}>
                <Text color={isCursor ? "blue" : "white"} bold={isCursor}>
                  {entry.name}
                </Text>
              </Box>
              <Box width={7}><Text color="gray">{typePart}</Text></Box>
              <Box width={9}>
                <Text color={kind === "built-in" ? "cyan" : "magenta"}>{kind}</Text>
              </Box>
              <Box width={11}>
                <Text color={source === "official" ? "yellow" : "gray"}>{source}</Text>
              </Box>
              <Text dimColor wrap="truncate">{desc}</Text>
            </Box>
          );
        }

        return (
          <Box key={entry.name}>
            <Box width={3}>
              <Text color={isCursor ? "blue" : undefined}>
                {isCursor ? " ›" : "  "}
              </Text>
            </Box>
            <Box width={nameWidth}>
              <Text color={isCursor ? "blue" : "white"} bold={isCursor}>
                {entry.name}
              </Text>
            </Box>
            <Text color={statusColor}>{statusText}</Text>
          </Box>
        );
      })}

      {/* Scroll indicator below (always reserve space) */}
      <Box>
        <Text dimColor>{hiddenBelow > 0 ? `  ↓ ${hiddenBelow} more below` : " "}</Text>
      </Box>

      {/* Empty state for project/global */}
      {filteredByTab.length === 0 && activeTab !== "registry" && activeTab !== "agents" && (
        <Box>
          <Text dimColor>  No MCPs configured. Press Ctrl+A to add from registry.</Text>
        </Box>
      )}

      {/* Footer */}
      <Box marginTop={1} gap={2}>
        {activeTab === "agents" ? (
          <>
            <Text dimColor>Space</Text><Text>toggle</Text>
          </>
        ) : activeTab === "registry" ? (
          <>
            <Text dimColor>Enter</Text><Text>apply</Text>
            <Text dimColor>Ctrl+A</Text><Text>new</Text>
            <Text dimColor>e</Text><Text>edit</Text>
            <Text dimColor>d</Text><Text>delete</Text>
          </>
        ) : (
          <>
            <Text dimColor>Ctrl+A</Text><Text>add</Text>
            <Text dimColor>d</Text><Text>remove</Text>
            <Text dimColor>Enter</Text><Text>configure</Text>
          </>
        )}
        <Text dimColor>Ctrl+S</Text><Text>apply</Text>
        <Text dimColor>←→</Text><Text>tab</Text>
        <Text dimColor>Tab</Text><Text>search</Text>
        <Text dimColor>q</Text><Text>quit</Text>
      </Box>
      {message && (
        <Box marginTop={1}>
          <Text color="green" bold>{message}</Text>
        </Box>
      )}
    </Box>
  );
}
