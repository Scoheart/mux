import { useState } from "react";
import { Box, Text, useInput } from "ink";
import type { DiffEntry } from "../types.js";

interface Props {
  diffs: DiffEntry[];
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmView({ diffs, onConfirm, onCancel }: Props) {
  const options = ["Apply", "Cancel"] as const;
  const [cursor, setCursor] = useState(0);

  useInput((_input, key) => {
    if (key.leftArrow) {
      setCursor(0);
    }
    if (key.rightArrow) {
      setCursor(1);
    }
    if (key.return) {
      if (cursor === 0) onConfirm();
      else onCancel();
    }
    if (key.escape) {
      onCancel();
    }
  });

  const adds = diffs.filter((d) => d.action === "add");
  const removes = diffs.filter((d) => d.action === "remove");

  return (
    <Box flexDirection="column">
      <Box flexDirection="column" borderStyle="round" paddingX={1} borderColor="yellow">
        <Box marginBottom={1}>
          <Text bold color="yellow">⚠ Review changes ({diffs.length} total):</Text>
        </Box>

        <Box flexDirection="column">
          {adds.map((d, i) => (
            <Box key={`a${i}`} gap={1}>
              <Text color="green" bold>+</Text>
              <Text color="green">{d.mcpName}</Text>
              <Text dimColor>→</Text>
              <Text>{d.agent}</Text>
              <Text dimColor>[{d.scope}]</Text>
            </Box>
          ))}
          {removes.map((d, i) => (
            <Box key={`r${i}`} gap={1}>
              <Text color="red" bold>-</Text>
              <Text color="red">{d.mcpName}</Text>
              <Text dimColor>←</Text>
              <Text>{d.agent}</Text>
              <Text dimColor>[{d.scope}]</Text>
            </Box>
          ))}
        </Box>
      </Box>

      <Box marginTop={1} gap={2}>
        {options.map((opt, i) => (
          <Text
            key={opt}
            color={i === cursor ? (i === 0 ? "green" : "red") : "gray"}
            bold={i === cursor}
          >
            {i === cursor ? "› " : "  "}{opt}
          </Text>
        ))}
      </Box>
    </Box>
  );
}
