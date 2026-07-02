import { useState } from "react";
import { Box, Text, useInput, useStdout } from "ink";
import TextInput from "ink-text-input";
import type { RegistryEntry, McpStdioConfig, McpHttpConfig } from "../types.js";
import { ShimmerBorder } from "./shimmer-border.js";

interface Props {
  entry: RegistryEntry;
  onSave: (entry: RegistryEntry) => void;
  onCancel: () => void;
}

type McpType = "stdio" | "http";

interface FormState {
  description: string;
  tags: string;
  mcpType: McpType;
  command: string;
  args: string;
  env: string;
  url: string;
  httpType: string;
  headers: string;
}

const FIELDS_STDIO = ["description", "tags", "mcpType", "command", "args", "env"] as const;
const FIELDS_HTTP = ["description", "tags", "mcpType", "url", "httpType", "headers"] as const;

function entryToForm(entry: RegistryEntry): FormState {
  const mcpType: McpType = entry.config.http ? "http" : "stdio";
  const stdio = entry.config.stdio;
  const http = entry.config.http;
  return {
    description: entry.description || "",
    tags: entry.tags?.join(", ") || "",
    mcpType,
    command: stdio?.command || "",
    args: stdio?.args?.join(", ") || "",
    env: stdio?.env ? Object.entries(stdio.env).map(([k, v]) => `${k}=${v}`).join(", ") : "",
    url: http?.url || "",
    httpType: http?.type || "http",
    headers: http?.headers ? Object.entries(http.headers).map(([k, v]) => `${k}=${v}`).join(", ") : "",
  };
}

function formToEntry(name: string, form: FormState): RegistryEntry {
  const entry: RegistryEntry = {
    name,
    description: form.description,
    tags: form.tags ? form.tags.split(",").map((t) => t.trim()).filter(Boolean) : [],
    config: {},
  };

  if (form.mcpType === "stdio") {
    const stdio: McpStdioConfig = {
      command: form.command,
      args: form.args ? form.args.split(",").map((a) => a.trim()).filter(Boolean) : [],
    };
    if (form.env) {
      stdio.env = {};
      for (const pair of form.env.split(",")) {
        const [key, ...rest] = pair.split("=");
        if (key?.trim()) {
          stdio.env[key.trim()] = rest.join("=").trim();
        }
      }
    }
    entry.config.stdio = stdio;
  } else {
    const http: McpHttpConfig = {
      type: form.httpType,
      url: form.url,
    };
    if (form.headers) {
      http.headers = {};
      for (const pair of form.headers.split(",")) {
        const [key, ...rest] = pair.split("=");
        if (key?.trim()) {
          http.headers[key.trim()] = rest.join("=").trim();
        }
      }
    }
    entry.config.http = http;
  }

  return entry;
}

function getFieldLabel(field: string): string {
  const labels: Record<string, string> = {
    description: "Description",
    tags: "Tags (comma-separated)",
    mcpType: "Type",
    command: "Command",
    args: "Args (comma-separated)",
    env: "Env (KEY=val, ...)",
    url: "URL",
    httpType: "HTTP Type",
    headers: "Headers (Key=val, ...)",
  };
  return labels[field] || field;
}

export function RegistryEditView({ entry, onSave, onCancel }: Props) {
  const [form, setForm] = useState<FormState>(entryToForm(entry));
  const [cursor, setCursor] = useState(0);
  const [editing, setEditing] = useState(false);
  const { stdout } = useStdout();
  const panelWidth = stdout?.columns ?? 80;

  const fields = form.mcpType === "stdio" ? FIELDS_STDIO : FIELDS_HTTP;
  const contentRows = fields.length + 4;

  useInput((input, key) => {
    if (editing) {
      if (key.return || key.escape) {
        setEditing(false);
      }
      return;
    }

    if (key.escape) {
      onCancel();
      return;
    }

    if (key.upArrow) {
      setCursor((c) => Math.max(0, c - 1));
      return;
    }
    if (key.downArrow) {
      setCursor((c) => Math.min(fields.length - 1, c + 1));
      return;
    }

    if (key.return) {
      const field = fields[cursor];
      if (field === "mcpType") {
        setForm((f) => ({ ...f, mcpType: f.mcpType === "stdio" ? "http" : "stdio" }));
        return;
      }
      setEditing(true);
      return;
    }

    if (input === "s" && key.ctrl) {
      onSave(formToEntry(entry.name, form));
      return;
    }
  });

  const handleFieldChange = (field: string, value: string) => {
    setForm((f) => ({ ...f, [field]: value }));
  };

  return (
    <ShimmerBorder width={panelWidth} contentRows={contentRows} preset="yellow">
      <Box flexDirection="column" paddingX={1}>
        <Box marginBottom={1} gap={1}>
          <Text bold color="yellow">Edit:</Text>
          <Text bold>{entry.name}</Text>
        </Box>

        <Box flexDirection="column">
          {fields.map((field, i) => {
            const isCursor = cursor === i;
            const isEditing = editing && isCursor;
            const value = form[field];

            return (
              <Box key={field} gap={1}>
                <Text color={isCursor ? "yellow" : undefined}>
                  {isCursor ? "›" : " "}
                </Text>
                <Box width={24}>
                  <Text color={isCursor ? "yellow" : "gray"}>{getFieldLabel(field)}:</Text>
                </Box>
                {field === "mcpType" ? (
                  <Text color="cyan">{value} <Text dimColor>(Enter to toggle)</Text></Text>
                ) : isEditing ? (
                  <TextInput
                    value={String(value)}
                    onChange={(v) => handleFieldChange(field, v)}
                    focus={true}
                  />
                ) : (
                  <Text color={value ? "white" : "gray"}>{value || "—"}</Text>
                )}
              </Box>
            );
          })}
        </Box>

        <Box marginTop={1} gap={2}>
          <Text dimColor>↑↓</Text><Text>navigate</Text>
          <Text dimColor>Enter</Text><Text>edit field</Text>
          <Text dimColor>Ctrl+S</Text><Text>save</Text>
          <Text dimColor>Esc</Text><Text>cancel</Text>
        </Box>
      </Box>
    </ShimmerBorder>
  );
}

export { formToEntry, entryToForm, type FormState, type McpType };
