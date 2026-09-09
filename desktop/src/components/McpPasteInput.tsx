const EXAMPLE = `{
  "mcpServers": {
    "my-server": {
      "command": "npx",
      "args": ["-y", "my-mcp-server"]
    }
  }
}`;

export function McpPasteInput({ value, onChange, disabled }: {
  value: string;
  onChange(value: string): void;
  disabled: boolean;
}) {
  return <div className="mux-mcp-paste-fields">
    <p>支持 JSON、TOML、YAML，可直接粘贴从 MUX 复制的配置。</p>
    <textarea
      autoFocus
      aria-label="MCP 配置"
      className="mux-paste-config-input"
      placeholder={EXAMPLE}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      disabled={disabled}
      spellCheck={false}
      autoCapitalize="off"
      autoCorrect="off"
    />
  </div>;
}
