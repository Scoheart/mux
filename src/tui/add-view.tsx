import { useState, useMemo, useRef } from "react";
import { Box, Text, useInput } from "ink";
import TextInput from "ink-text-input";
import { createMcpSearcher } from "../utils/fuzzy.js";
import type { RegistryEntry } from "../types.js";

interface Props {
  entries: RegistryEntry[];
  alreadyAdded: string[];
  scopeLabel: string;
  onSelect: (name: string) => void;
  onCancel: () => void;
}

const MAX_VISIBLE = 15;

export function AddView({ entries, alreadyAdded, scopeLabel, onSelect, onCancel }: Props) {
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const [focusArea, setFocusArea] = useState<"search" | "list">("search");
  const scrollOffset = useRef(0);

  // Only show MCPs not already added in this scope
  const available = useMemo(
    () => entries.filter((e) => !alreadyAdded.includes(e.name)),
    [entries, alreadyAdded]
  );

  const searcher = useMemo(() => createMcpSearcher(available), [available]);
  const filtered = useMemo(() => searcher(query), [searcher, query]);

  // Scroll tracking
  if (cursor < scrollOffset.current) {
    scrollOffset.current = cursor;
  } else if (cursor >= scrollOffset.current + MAX_VISIBLE) {
    scrollOffset.current = cursor - MAX_VISIBLE + 1;
  }

  const visibleItems = filtered.slice(scrollOffset.current, scrollOffset.current + MAX_VISIBLE);
  const hiddenBelow = Math.max(0, filtered.length - scrollOffset.current - MAX_VISIBLE);
  const hiddenAbove = scrollOffset.current;

  useInput((input, key) => {
    if (key.escape) {
      onCancel();
      return;
    }

    if (focusArea === "search" && key.downArrow) {
      setFocusArea("list");
      return;
    }

    if (focusArea === "list") {
      if (key.upArrow) {
        if (cursor === 0) {
          setFocusArea("search");
        } else {
          setCursor((c) => c - 1);
        }
        return;
      }
      if (key.downArrow) {
        setCursor((c) => Math.min(filtered.length - 1, c + 1));
        return;
      }
      if (key.return) {
        const item = filtered[cursor];
        if (item) onSelect(item.name);
        return;
      }
    }
  });

  const handleQueryChange = (value: string) => {
    setQuery(value);
    setCursor(0);
    scrollOffset.current = 0;
  };

  return (
    <Box flexDirection="column" borderStyle="round" borderColor="green" paddingX={1}>
      <Box marginBottom={1}>
        <Text bold color="green">Add MCP to {scopeLabel}</Text>
        <Text dimColor>  ({available.length} available)</Text>
      </Box>

      <Box marginBottom={1}>
        <Text color={focusArea === "search" ? "cyan" : "gray"}>🔍 </Text>
        {focusArea === "search" ? (
          <TextInput
            value={query}
            onChange={handleQueryChange}
            placeholder="Filter..."
          />
        ) : (
          <Text dimColor>{query || "Filter..."}</Text>
        )}
      </Box>

      {/* Scroll indicator above */}
      <Box>
        <Text dimColor>{hiddenAbove > 0 ? `  ↑ ${hiddenAbove} more above` : " "}</Text>
      </Box>

      {/* Fixed height list */}
      {Array.from({ length: MAX_VISIBLE }, (_, i) => {
        const entry = visibleItems[i];
        if (!entry) {
          return <Box key={`empty-${i}`}><Text> </Text></Box>;
        }
        const actualIndex = i + scrollOffset.current;
        const isCursor = actualIndex === cursor && focusArea === "list";
        return (
          <Box key={entry.name} gap={1}>
            <Text color={isCursor ? "green" : undefined}>
              {isCursor ? "›" : " "}
            </Text>
            <Text color={isCursor ? "green" : "white"} bold={isCursor}>
              {entry.name}
            </Text>
          </Box>
        );
      })}

      {/* Scroll indicator below */}
      <Box>
        <Text dimColor>{hiddenBelow > 0 ? `  ↓ ${hiddenBelow} more below` : " "}</Text>
      </Box>

      {filtered.length === 0 && (
        <Text dimColor>  No MCPs available to add.</Text>
      )}

      <Box marginTop={1} gap={2}>
        <Text dimColor>↑↓</Text><Text>navigate</Text>
        <Text dimColor>Enter</Text><Text>select</Text>
        <Text dimColor>Esc</Text><Text>cancel</Text>
      </Box>
    </Box>
  );
}
