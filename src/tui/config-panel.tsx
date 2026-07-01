import { useState } from "react";
import { Box, Text, useInput } from "ink";

interface Props {
  mcpName: string;
  scopeLabel: string;
  currentAgents: string[];
  availableAgents: string[];
  onSave: (targets: string[]) => void;
  onCancel: () => void;
}

export function ConfigPanel({ mcpName, scopeLabel, currentAgents, availableAgents, onSave, onCancel }: Props) {
  const [targets, setTargets] = useState<Set<string>>(new Set(currentAgents));
  const [cursor, setCursor] = useState(0);

  useInput((input, key) => {
    if (key.escape) { onCancel(); return; }

    if (key.return || (input === "s" && key.ctrl)) {
      onSave([...targets]);
      return;
    }

    if (key.upArrow) {
      setCursor((c) => Math.max(0, c - 1));
      return;
    }
    if (key.downArrow) {
      setCursor((c) => Math.min(availableAgents.length - 1, c + 1));
      return;
    }

    if (input === " ") {
      const targetName = availableAgents[cursor];
      setTargets((prev) => {
        const next = new Set(prev);
        if (next.has(targetName)) next.delete(targetName);
        else next.add(targetName);
        return next;
      });
    }

    // Ctrl+A to toggle all
    if (input === "a" && key.ctrl) {
      const allSelected = availableAgents.every((t) => targets.has(t));
      if (allSelected) {
        setTargets(new Set());
      } else {
        setTargets(new Set(availableAgents));
      }
    }
  });

  return (
    <Box flexDirection="column" borderStyle="round" paddingX={1} borderColor="cyan">
      <Box marginBottom={1} gap={1}>
        <Text bold color="cyan">{mcpName}</Text>
        <Text dimColor>({scopeLabel})</Text>
      </Box>

      <Text dimColor>  Select targets to enable:</Text>
      <Box flexDirection="column" marginTop={1}>
        {availableAgents.map((t, i) => {
          const isCursor = cursor === i;
          const isChecked = targets.has(t);
          return (
            <Box key={t} gap={1}>
              <Text color={isCursor ? "cyan" : undefined}>
                {isCursor ? "›" : " "}
              </Text>
              <Text color={isChecked ? "green" : "gray"}>
                {isChecked ? "◉" : "○"}
              </Text>
              <Text color={isCursor ? "cyan" : "white"}>{t}</Text>
            </Box>
          );
        })}
      </Box>

      <Box marginTop={1} gap={2}>
        <Text dimColor>↑↓</Text><Text>navigate</Text>
        <Text dimColor>Space</Text><Text>toggle</Text>
        <Text dimColor>Ctrl+A</Text><Text>all</Text>
        <Text dimColor>Enter</Text><Text>save</Text>
        <Text dimColor>Esc</Text><Text>cancel</Text>
      </Box>
    </Box>
  );
}
