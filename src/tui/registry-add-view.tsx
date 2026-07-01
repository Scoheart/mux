import { useState } from "react";
import { Box, Text, useInput } from "ink";
import TextInput from "ink-text-input";
import type { RegistryEntry, McpStdioConfig, McpHttpConfig } from "../types.js";

interface Props {
  existingNames: string[];
  onSave: (entry: RegistryEntry) => void;
  onCancel: () => void;
}

type McpType = "stdio" | "http";

interface FormState {
  name: string;
  description: string;
  tags: string;
  mcpType: McpType;
  command: string;
  args: string;
  env: string;
  url: string;
  httpType: "http" | "sse";
  headers: string;
}

const FIELDS_STDIO = ["name", "description", "tags", "mcpType", "command", "args", "env"] as const;
const FIELDS_HTTP = ["name", "description", "tags", "mcpType", "url", "httpType", "headers"] as const;

function getFieldLabel(field: string): string {
  const labels: Record<string, string> = {
    name: "Name",
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

function formToEntry(form: FormState): RegistryEntry {
  const entry: RegistryEntry = {
    name: form.name,
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

export function RegistryAddView({ existingNames, onSave, onCancel }: Props) {
  const [form, setForm] = useState<FormState>({
    name: "",
    description: "",
    tags: "",
    mcpType: "stdio",
    command: "",
    args: "",
    env: "",
    url: "",
    httpType: "http",
    headers: "",
  });
  const [cursor, setCursor] = useState(0);
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState("");

  const fields = form.mcpType === "stdio" ? FIELDS_STDIO : FIELDS_HTTP;

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
      if (field === "httpType") {
        setForm((f) => ({ ...f, httpType: f.httpType === "http" ? "sse" : "http" }));
        return;
      }
      setEditing(true);
      return;
    }

    if (input === "s" && key.ctrl) {
      if (!form.name.trim()) {
        setError("Name is required");
        return;
      }
      if (existingNames.includes(form.name.trim())) {
        setError(`"${form.name}" already exists`);
        return;
      }
      if (form.mcpType === "stdio" && !form.command.trim()) {
        setError("Command is required for stdio type");
        return;
      }
      if (form.mcpType === "http" && !form.url.trim()) {
        setError("URL is required for http type");
        return;
      }
      setError("");
      onSave(formToEntry(form));
      return;
    }
  });

  const handleFieldChange = (field: string, value: string) => {
    setForm((f) => ({ ...f, [field]: value }));
    if (error) setError("");
  };

  return (
    <Box flexDirection="column" borderStyle="round" borderColor="green" paddingX={1}>
      <Box marginBottom={1}>
        <Text bold color="green">Add New MCP Server</Text>
      </Box>

      <Box flexDirection="column">
        {fields.map((field, i) => {
          const isCursor = cursor === i;
          const isEditing = editing && isCursor;
          const value = form[field];

          return (
            <Box key={field} gap={1}>
              <Text color={isCursor ? "green" : undefined}>
                {isCursor ? "›" : " "}
              </Text>
              <Box width={24}>
                <Text color={isCursor ? "green" : "gray"}>{getFieldLabel(field)}:</Text>
              </Box>
              {field === "mcpType" ? (
                <Text color="cyan">{value} <Text dimColor>(Enter to toggle)</Text></Text>
              ) : field === "httpType" ? (
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

      {error && (
        <Box marginTop={1}>
          <Text color="red">✗ {error}</Text>
        </Box>
      )}

      <Box marginTop={1} gap={2}>
        <Text dimColor>↑↓</Text><Text>navigate</Text>
        <Text dimColor>Enter</Text><Text>edit field</Text>
        <Text dimColor>Ctrl+S</Text><Text>save</Text>
        <Text dimColor>Esc</Text><Text>cancel</Text>
      </Box>
    </Box>
  );
}
