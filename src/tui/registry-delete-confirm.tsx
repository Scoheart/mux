import { Box, Text, useInput } from "ink";

interface Props {
  mcpName: string;
  onConfirm: (alsoRemoveFromAgents: boolean) => void;
  onCancel: () => void;
}

export function RegistryDeleteConfirm({ mcpName, onConfirm, onCancel }: Props) {
  useInput((input, key) => {
    if (key.escape || input === "n") {
      onCancel();
      return;
    }
    if (input === "y") {
      onConfirm(true);
      return;
    }
    if (key.return) {
      onConfirm(false);
      return;
    }
  });

  return (
    <Box flexDirection="column" borderStyle="round" borderColor="red" paddingX={1}>
      <Box marginBottom={1}>
        <Text bold color="red">Delete MCP</Text>
      </Box>

      <Box flexDirection="column" gap={1}>
        <Text>
          Are you sure you want to delete <Text bold color="yellow">{mcpName}</Text> from the registry?
        </Text>
        <Text dimColor>
          This will remove the MCP definition. It will NOT automatically remove it from target tool configs.
        </Text>
      </Box>

      <Box marginTop={1} gap={2}>
        <Text dimColor>y</Text><Text>delete + remove from all targets</Text>
      </Box>
      <Box gap={2}>
        <Text dimColor>Enter</Text><Text>delete registry only</Text>
      </Box>
      <Box gap={2}>
        <Text dimColor>n/Esc</Text><Text>cancel</Text>
      </Box>
    </Box>
  );
}
