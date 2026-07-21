#!/usr/bin/env node

import { execFile } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { promisify } from "node:util";
import { fileURLToPath, pathToFileURL } from "node:url";

const execFileAsync = promisify(execFile);
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const output = resolve(root, "analysis/agent-capability-audit/repository-evidence-snippets.json");
const lineSignal = /(?:~\/|\$HOME|home_dir|config_dir|config\.toml|settings\.json|mcpServers|mcp_servers|mcp\.providers|model.?context.?protocol|custom_providers|default_model|default_text_model|active_model|provider\s*[=:]|model\s*[=:]|api_key_env|keyring|skills?[/_.-]|SKILL\.md)/i;
const sensitiveName = String.raw`(?:api[_-]?key|access[_-]?key(?:[_-]?id)?|secret[_-]?access[_-]?key|(?:access|refresh|id)?[_-]?token|client[_-]?secret|password|passwd|authorization)`;
const sensitiveIdentifier = String.raw`[A-Za-z0-9_-]*${sensitiveName}`;
const sensitiveAssignmentStart = new RegExp(
  `(^|[^A-Za-z0-9_-])(["']?${sensitiveIdentifier}["']?)(\\s*[:=])`,
  "i",
);
const credentialLeakChecks = [
  ["private-key", /-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----/],
  ["openai-style-key", /\b(?:sk|rk|pk)-[A-Za-z0-9_-]{12,}\b/],
  ["token-plan-key", /\btp-[A-Za-z0-9_-]{16,}\b/],
  ["huggingface-token", /\bhf_[A-Za-z0-9]{20,}\b/],
  ["groq-key", /\bgsk_[A-Za-z0-9]{20,}\b/],
  ["xai-key", /\bxai-[A-Za-z0-9_-]{20,}\b/],
  ["github-token", /\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b/],
  ["aws-access-key", /\b(?:AKIA|ASIA)[0-9A-Z]{16}\b/],
  ["google-api-key", /\bAIza[0-9A-Za-z_-]{30,}\b/],
  ["slack-token", /\bxox[baprs]-[0-9A-Za-z-]{20,}\b/],
  ["jwt", /\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b/],
  ["bearer-token", /\bBearer\s+[A-Za-z0-9._~+\/-]{12,}\b/i],
  ["url-userinfo", /https?:\/\/[^\/@\s]+:[^\/@\s]+@/i],
  ["sensitive-assignment", new RegExp(`(?:["']?${sensitiveName}["']?)\\s*[:=]\\s*["']?[A-Za-z0-9_.\\/+~=-]{12,}`, "i")],
];

function redactSensitiveAssignmentLine(line) {
  const match = sensitiveAssignmentStart.exec(line);
  if (!match) return line;
  const valueStart = match.index + match[0].length;
  return `${line.slice(0, valueStart)} [redacted-example]`;
}

export function redact(text) {
  const shapeRedacted = text
    .replace(/-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----[\s\S]*?(?:-----END (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----|$)/g, "[redacted-private-key]")
    .replace(/\b(sk|rk|pk)-[A-Za-z0-9_-]{12,}\b/g, "$1-[redacted-example]")
    .replace(/\btp-[A-Za-z0-9_-]{16,}\b/g, "tp-[redacted-example]")
    .replace(/\bhf_[A-Za-z0-9]{20,}\b/g, "hf_[redacted-example]")
    .replace(/\bgsk_[A-Za-z0-9]{20,}\b/g, "gsk_[redacted-example]")
    .replace(/\bxai-[A-Za-z0-9_-]{20,}\b/g, "xai-[redacted-example]")
    .replace(/\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b/g, "[redacted-github-token]")
    .replace(/\b(?:AKIA|ASIA)[0-9A-Z]{16}\b/g, "[redacted-aws-access-key]")
    .replace(/\bAIza[0-9A-Za-z_-]{30,}\b/g, "[redacted-google-api-key]")
    .replace(/\bxox[baprs]-[0-9A-Za-z-]{20,}\b/g, "[redacted-slack-token]")
    .replace(/\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b/g, "[redacted-jwt]")
    .replace(/(\bBearer\s+)[A-Za-z0-9._~+\/-]{12,}\b/gi, "$1[redacted-token]")
    .replace(/(https?:\/\/[^:/@\s]+:)[^@/\s]+(@)/gi, "$1[redacted-password]$2");
  return shapeRedacted
    .split(/(\r?\n)/)
    .map((part) => (/^\r?\n$/.test(part) ? part : redactSensitiveAssignmentLine(part)))
    .join("");
}

export function assertNoCredentialLikeText(text) {
  const hit = credentialLeakChecks.find(([, pattern]) => pattern.test(text));
  if (hit) {
    throw new Error(`refusing to persist credential-shaped evidence (${hit[0]})`);
  }
}

function assertPayloadHasNoCredentialLikeText(value) {
  if (typeof value === "string") {
    assertNoCredentialLikeText(value);
    return;
  }
  if (Array.isArray(value)) {
    value.forEach(assertPayloadHasNoCredentialLikeText);
    return;
  }
  if (value && typeof value === "object") {
    Object.values(value).forEach(assertPayloadHasNoCredentialLikeText);
  }
}

export function serializeSnapshot(payload) {
  assertPayloadHasNoCredentialLikeText(payload);
  const serialized = JSON.stringify(payload, null, 2);
  assertNoCredentialLikeText(serialized);
  return `${serialized}\n`;
}

export function safeFetchError(error) {
  if (String(error?.message ?? "").startsWith("unsupported blob encoding")) {
    return "unsupported-blob-encoding";
  }
  const code = String(error?.code ?? "unknown").replace(/[^A-Za-z0-9_.-]/g, "").slice(0, 32);
  return `gh-api-failed:${code || "unknown"}`;
}

export function snippets(content) {
  const lines = content.split(/\r?\n/);
  const ranges = [];
  for (let index = 0; index < lines.length; index += 1) {
    if (!lineSignal.test(lines[index])) continue;
    const start = Math.max(0, index - 2);
    const end = Math.min(lines.length, index + 3);
    const previous = ranges.at(-1);
    if (previous && start <= previous.end + 1) previous.end = Math.max(previous.end, end);
    else ranges.push({ start, end });
    if (ranges.length >= 24) break;
  }
  return ranges.map(({ start, end }) => ({
    startLine: start + 1,
    endLine: end,
    text: redact(lines.slice(start, end).join("\n")).slice(0, 6000),
  }));
}

async function blob(repository, sha) {
  let stdout;
  let lastError;
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    try {
      ({ stdout } = await execFileAsync("gh", ["api", `repos/${repository}/git/blobs/${sha}`], {
        maxBuffer: 8 * 1024 * 1024,
      }));
      break;
    } catch (error) {
      lastError = error;
      if (attempt < 3) {
        await new Promise((resolveDelay) => setTimeout(resolveDelay, 250 * 2 ** (attempt - 1)));
      }
    }
  }
  if (stdout === undefined) throw lastError;
  const payload = JSON.parse(stdout);
  if (payload.encoding !== "base64") throw new Error(`unsupported blob encoding: ${payload.encoding}`);
  return Buffer.from(payload.content.replaceAll("\n", ""), "base64").toString("utf8");
}

export async function main() {
  const scans = JSON.parse(await readFile(resolve(root, "analysis/agent-capability-audit/repository-tree-scans.json"), "utf8"));
  const repositories = scans.records.sort((left, right) => left.repository.localeCompare(right.repository));
  const results = new Array(repositories.length);
  let cursor = 0;
  let completed = 0;

  async function persist() {
    const records = results.filter(Boolean).sort((left, right) => left.repository.localeCompare(right.repository));
    await mkdir(dirname(output), { recursive: true });
    await writeFile(output, serializeSnapshot({
      schemaVersion: 1,
      generatedAt: new Date().toISOString(),
      totalRepositories: repositories.length,
      completedRepositories: records.length,
      records,
    }));
  }

  async function worker() {
    while (cursor < repositories.length) {
      const index = cursor;
      cursor += 1;
      const repository = repositories[index];
      const files = [];
      for (const candidate of repository.evidenceCandidates.slice(0, 8)) {
        try {
          const content = await blob(repository.repository, candidate.sha);
          const extracted = snippets(content);
          files.push({
            ...candidate,
            blobUrl: `https://github.com/${repository.repository}/blob/${repository.commit}/${candidate.path}`,
            extractionStatus: extracted.length > 0 ? "matched" : "no-matching-lines",
            snippets: extracted,
            error: null,
          });
        } catch (error) {
          files.push({
            ...candidate,
            blobUrl: `https://github.com/${repository.repository}/blob/${repository.commit}/${candidate.path}`,
            extractionStatus: "fetch-failed",
            snippets: [],
            error: safeFetchError(error),
          });
        }
      }
      results[index] = {
        repository: repository.repository,
        agentIds: repository.agentIds,
        commit: repository.commit,
        files,
      };
      completed += 1;
      if (completed % 5 === 0 || completed === repositories.length) {
        await persist();
        console.log(JSON.stringify({ completed, total: repositories.length, repository: repository.repository }));
      }
    }
  }

  await Promise.all(Array.from({ length: 4 }, () => worker()));
  await persist();
  const files = results.flatMap((record) => record.files);
  console.log(JSON.stringify({
    output,
    repositories: results.length,
    files: files.length,
    matchedFiles: files.filter((file) => file.extractionStatus === "matched").length,
    failedFiles: files.filter((file) => file.extractionStatus === "fetch-failed").length,
  }));
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  await main();
}
