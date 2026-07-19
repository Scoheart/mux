import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const SOURCE = "https://glama.ai/mcp/clients";
const VERIFIED_AT =
  process.env.MUX_CATALOG_DATE ||
  new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Shanghai",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(new Date());

// Collapse directory aliases onto MUX's stable ids. Distinct product surfaces
// (for example Amazon Q IDE versus its CLI) intentionally remain separate.
const ALIASES = {
  "chat-gpt": "chatgpt",
  "vs-code-github-copilot": "vscode",
  "lm-studio": "lmstudio",
  "amazon-q-ide": "amazon-q",
  "augment-code": "augment",
  "gemini-cli": "gemini",
  "jetbrains-junie": "junie",
};

const SUPPLEMENTAL = {
  "blackbox-cli": ["BLACKBOX CLI", "https://docs.blackbox.ai/blackbox-ai-1/blackbox-cli/mcp-server", "cli"],
  "chatgpt": ["ChatGPT", "https://help.openai.com/en/articles/11487775-connectors-in-chatgpt", "web"],
  "claude-ai": ["Claude.ai", "https://support.claude.com/en/articles/11175166-getting-started-with-custom-connectors-using-remote-mcp", "web"],
  "copilot-xcode": ["GitHub Copilot for Xcode", "https://docs.github.com/en/copilot/customizing-copilot/extending-copilot-chat-with-mcp", "ide"],
  "docker-agent": ["Docker Agent", "https://docs.docker.com/ai/cagent/", "cli"],
  "docker-gordon": ["Docker Gordon", "https://docs.docker.com/ai/gordon/", "desktop"],
  "jetbrains-air": ["JetBrains Air", "https://www.jetbrains.com/help/air/mcp-servers.html", "ide"],
  "lovable": ["Lovable", "https://docs.lovable.dev/integrations/mcp-servers", "web"],
  "microsoft-365-copilot": ["Microsoft 365 Copilot", "https://learn.microsoft.com/en-us/microsoft-365-copilot/extensibility/overview-mcp", "web"],
  "open-webui": ["Open WebUI", "https://docs.openwebui.com/features/extensibility/mcp/", "web"],
  "posthog-code": ["PostHog Code", "https://posthog.com/docs/code", "coding-agent"],
  "qodo": ["Qodo", "https://docs.qodo.ai/qodo-documentation/qodo-merge/integrations/mcp", "coding-agent"],
  "raycast": ["Raycast", "https://manual.raycast.com/ai/ai-extensions", "desktop"],
  "replit-agent": ["Replit Agent", "https://docs.replit.com/replitai/integrations/mcp", "web"],
  "sema4": ["Sema4.ai", "https://sema4.ai/docs/build-agents/mcp", "agent-platform"],
  "trae-agent": ["TRAE Agent", "https://github.com/bytedance/TRAE-agent", "cli"],
  "trae-ide": ["TRAE IDE", "https://docs.trae.ai/ide/model-context-protocol", "ide"],
  "visual-studio": ["Visual Studio", "https://learn.microsoft.com/en-us/visualstudio/ide/mcp-servers", "ide"],
};

function entry(name, docs, category = "client", evidence = "catalog") {
  return {
    global: null,
    project: null,
    format: "unknown",
    key: "",
    enabled: true,
    builtin: true,
    name,
    docs,
    note: "未确认稳定的用户级全局配置文件；MUX 仅展示，不写入。",
    category,
    evidence,
    verified_at: VERIFIED_AT,
    codec: "standard",
    layout: "map",
    transports: [],
  };
}

const response = await fetch(SOURCE, { headers: { "user-agent": "mux-agent-catalog/1" } });
if (!response.ok) throw new Error(`Glama request failed: ${response.status}`);
const html = await response.text();
const pattern = /href="\/mcp\/clients\/([^\"]+)"[^>]*>([^<]+)/g;
const catalog = {};
for (const match of html.matchAll(pattern)) {
  const slug = match[1];
  const id = ALIASES[slug] || slug.toLowerCase();
  const name = match[2].replaceAll("&amp;", "&").trim();
  catalog[id] ||= entry(name, `${SOURCE}/${slug}`);
}

for (const [id, [name, docs, category]] of Object.entries(SUPPLEMENTAL)) {
  catalog[id] = entry(name, docs, category, "official");
}

if (Object.keys(catalog).length < 150) {
  throw new Error(`catalog unexpectedly small: ${Object.keys(catalog).length}`);
}

const sorted = Object.fromEntries(Object.entries(catalog).sort(([a], [b]) => a.localeCompare(b)));
const here = dirname(fileURLToPath(import.meta.url));
await writeFile(resolve(here, "../data/agent-catalog.json"), `${JSON.stringify(sorted, null, 2)}\n`);
console.log(`wrote ${Object.keys(sorted).length} catalog entries from ${SOURCE}`);
