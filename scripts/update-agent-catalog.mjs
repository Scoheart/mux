import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const SOURCE = "https://glama.ai/mcp/clients";
const ACP_REPOSITORY = "agentclientprotocol/registry";
// `verified_at` is an established AgentDefinition wire field. For discovery-only
// Glama entries it records when the catalog was collected; it is not evidence
// that an MCP / Models / Skills configuration contract was verified.
const CATALOG_COLLECTED_AT =
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

// ACP package ids describe launch surfaces. Collapse wrappers and CLI package
// names onto MUX's product identities without treating ACP support as proof of
// an MCP / Models / Skills file contract.
const ACP_ALIASES = {
  "amp-acp": "amp",
  auggie: "augment",
  "claude-acp": "claude-code",
  "codex-acp": "codex",
  "github-copilot-cli": "copilot-cli",
  kilo: "kilo-code",
  kimi: "kimi-code",
  "pi-acp": "pi",
  qoder: "qoder-cli",
  vtcode: "vt-code",
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
    verified_at: CATALOG_COLLECTED_AT,
    codec: "standard",
    layout: "map",
    transports: [],
  };
}

function acpEntry(manifest, manifestUrl) {
  const docs = manifest.repository || manifest.website || manifestUrl;
  return {
    ...entry(manifest.name, docs, "coding-agent", "acp-registry"),
    note: "ACP Registry 已确认该启动身份与分发方式，但尚未确认稳定的用户级 MCP、Models 或 Skills 写入契约；MUX 仅展示，不写入。",
  };
}

async function fetchJson(url) {
  const response = await fetch(url, {
    headers: {
      accept: "application/vnd.github+json",
      "user-agent": "mux-agent-catalog/1",
      "x-github-api-version": "2022-11-28",
    },
  });
  if (!response.ok) throw new Error(`${url} failed: ${response.status}`);
  return response.json();
}

async function mapWithConcurrency(items, limit, mapper) {
  const results = new Array(items.length);
  let cursor = 0;
  const workers = Array.from(
    { length: Math.min(limit, items.length) },
    async () => {
      while (cursor < items.length) {
        const index = cursor;
        cursor += 1;
        results[index] = await mapper(items[index], index);
      }
    },
  );
  await Promise.all(workers);
  return results;
}

async function fetchAcpManifests() {
  const repoApi = `https://api.github.com/repos/${ACP_REPOSITORY}`;
  const repository = await fetchJson(repoApi);
  const defaultBranch = repository.default_branch;
  if (typeof defaultBranch !== "string" || defaultBranch.length === 0) {
    throw new Error("ACP Registry response has no default branch");
  }

  // Resolve the moving default branch exactly once. Every tree lookup, raw
  // manifest fetch and human-auditable blob URL below is pinned to this commit.
  const commit = await fetchJson(`${repoApi}/commits/${encodeURIComponent(defaultBranch)}`);
  const commitSha = commit.sha;
  const treeSha = commit.commit?.tree?.sha;
  if (!/^[0-9a-f]{40}$/i.test(commitSha) || !/^[0-9a-f]{40}$/i.test(treeSha)) {
    throw new Error("ACP Registry response has an invalid commit or tree SHA");
  }

  const tree = await fetchJson(`${repoApi}/git/trees/${treeSha}?recursive=1`);
  if (tree.truncated) throw new Error("ACP Registry tree response is truncated");
  if (tree.sha !== treeSha) throw new Error("ACP Registry returned an unexpected tree SHA");
  const paths = tree.tree
    .filter((item) => item.type === "blob" && /^[^/]+\/agent\.json$/.test(item.path))
    .map((item) => item.path)
    .sort();
  return mapWithConcurrency(paths, 6, async (path) => {
    const manifestUrl = `https://github.com/${ACP_REPOSITORY}/blob/${commitSha}/${path}`;
    const rawUrl = `https://raw.githubusercontent.com/${ACP_REPOSITORY}/${commitSha}/${path}`;
    const manifest = await fetchJson(rawUrl);
    return { manifest, manifestUrl, commitSha };
  });
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

const acpManifests = await fetchAcpManifests();
for (const { manifest, manifestUrl } of acpManifests) {
  const id = ACP_ALIASES[manifest.id] || manifest.id;
  // A hand-maintained official supplemental entry carries stronger config
  // evidence than ACP launch metadata and must not be downgraded.
  if (catalog[id]?.evidence === "official") continue;
  catalog[id] = acpEntry(manifest, manifestUrl);
}

if (Object.keys(catalog).length < 180) {
  throw new Error(`catalog unexpectedly small: ${Object.keys(catalog).length}`);
}

const sorted = Object.fromEntries(Object.entries(catalog).sort(([a], [b]) => a.localeCompare(b)));
const here = dirname(fileURLToPath(import.meta.url));
await writeFile(resolve(here, "../data/agent-catalog.json"), `${JSON.stringify(sorted, null, 2)}\n`);
console.log(JSON.stringify({
  entries: Object.keys(sorted).length,
  glamaSource: SOURCE,
  glamaCollectedAt: CATALOG_COLLECTED_AT,
  glamaCollectionIsConfigContractVerification: false,
  acpRepository: ACP_REPOSITORY,
  acpRegistryCommit: acpManifests[0]?.commitSha ?? null,
}));
