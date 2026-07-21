#!/usr/bin/env node

import { execFile } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

const execFileAsync = promisify(execFile);
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const baseline = JSON.parse(await readFile(resolve(root, "analysis/agent-capability-audit/coverage-baseline.json"), "utf8"));
const output = resolve(root, "analysis/agent-capability-audit/repository-tree-scans.json");

function repoKey(url) {
  const match = url.match(/^https:\/\/github\.com\/([^/]+)\/([^/#?]+)/i);
  if (!match) return null;
  return `${match[1]}/${match[2].replace(/\.git$/, "")}`;
}

const refs = new Map();
for (const record of baseline.records) {
  for (const url of record.githubCandidates) {
    const repo = repoKey(url);
    if (!repo) continue;
    const ids = refs.get(repo) ?? new Set();
    ids.add(record.id);
    refs.set(repo, ids);
  }
}

const allowedExtension = /\.(?:md|mdx|txt|json|jsonc|toml|ya?ml|ts|tsx|js|jsx|mjs|cjs|py|rs|go|java|kt|swift|cs|rb|sh)$/i;
const signals = {
  mcp: /(^|[/_.-])mcp([/_.-]|$)|model.?context.?protocol/i,
  skills: /(^|[/_.-])skills?([/_.-]|$)|agent.?skills?/i,
  models: /(^|[/_.-])(models?|providers?)([/_.-]|$)|llm/i,
  config: /(^|[/_.-])(config|settings?|preferences?)([/_.-]|$)/i,
};

function classify(paths) {
  const result = {};
  for (const [capability, pattern] of Object.entries(signals)) {
    result[capability] = paths.filter((path) => pattern.test(path)).slice(0, 250);
  }
  return result;
}

function evidenceScore(path) {
  let score = 0;
  if (/(^|\/)(docs?|documentation|guides?|examples?)(\/|$)/i.test(path)) score += 35;
  if (/(^|\/)(mcp|skills?|models?|providers?|config|settings?)(\.[^/]+)?$/i.test(path)) score += 70;
  if (/(mcp|model.?context.?protocol|agent.?skills?|providers?|models?)/i.test(path)) score += 30;
  if (/\.(md|mdx|toml|ya?ml|jsonc?)$/i.test(path)) score += 20;
  if (/(^|\/)(src|crates?|packages?|apps?)(\/|$)/i.test(path)) score += 10;
  if (/(^|\/)(__tests__|tests?|fixtures?|snapshots?|generated|vendor|node_modules)(\/|$)|lock\./i.test(path)) score -= 80;
  return score;
}

function selectEvidenceCandidates(entries) {
  const selected = new Map();
  for (const [capability, pattern] of Object.entries(signals)) {
    const ranked = entries
      .filter((entry) => pattern.test(entry.path) && entry.size <= 512 * 1024)
      .map((entry) => ({ ...entry, capability, score: evidenceScore(entry.path) }))
      .filter((entry) => entry.score > 0)
      .sort((left, right) => right.score - left.score || left.path.localeCompare(right.path))
      .slice(0, 5);
    for (const entry of ranked) {
      const key = `${entry.sha}:${entry.path}`;
      const previous = selected.get(key);
      if (previous) {
        previous.capabilities = [...new Set([...previous.capabilities, capability])].sort();
      } else {
        selected.set(key, {
          path: entry.path,
          sha: entry.sha,
          size: entry.size,
          score: entry.score,
          capabilities: [capability],
        });
      }
    }
  }
  return [...selected.values()]
    .sort((left, right) => right.score - left.score || left.path.localeCompare(right.path))
    .slice(0, 14);
}

function safeScanError(error) {
  const code = String(error?.code ?? "unknown").replace(/[^A-Za-z0-9_.-]/g, "").slice(0, 32);
  return `gh-api-failed:${code || "unknown"}`;
}

const repositories = [...refs.entries()]
  .map(([repository, ids]) => ({ repository, agentIds: [...ids].sort() }))
  .sort((left, right) => left.repository.localeCompare(right.repository));
const results = new Array(repositories.length);
let cursor = 0;
let completed = 0;

async function persist() {
  const records = results.filter(Boolean).sort((left, right) => left.repository.localeCompare(right.repository));
  await mkdir(dirname(output), { recursive: true });
  await writeFile(output, `${JSON.stringify({
    schemaVersion: 1,
    generatedAt: new Date().toISOString(),
    totalRepositories: repositories.length,
    completedRepositories: records.length,
    records,
  }, null, 2)}\n`);
}

async function worker() {
  while (cursor < repositories.length) {
    const index = cursor;
    cursor += 1;
    const item = repositories[index];
    try {
      const { stdout } = await execFileAsync("gh", ["api", `repos/${item.repository}/git/trees/HEAD?recursive=1`], {
        maxBuffer: 64 * 1024 * 1024,
      });
      const tree = JSON.parse(stdout);
      const textEntries = tree.tree
        .filter((entry) => entry.type === "blob" && allowedExtension.test(entry.path))
        .map((entry) => ({ path: entry.path, sha: entry.sha, size: entry.size ?? 0 }))
        .sort((left, right) => left.path.localeCompare(right.path));
      const textPaths = textEntries.map((entry) => entry.path);
      const capabilityPaths = classify(textPaths);
      results[index] = {
        ...item,
        status: tree.truncated ? "tree-truncated" : "scanned",
        commit: tree.sha,
        treeEntries: tree.tree.length,
        textFiles: textPaths.length,
        capabilityPaths,
        evidenceCandidates: selectEvidenceCandidates(textEntries),
        error: null,
      };
    } catch (error) {
      results[index] = {
        ...item,
        status: "scan-failed",
        commit: null,
        treeEntries: 0,
        textFiles: 0,
        capabilityPaths: { mcp: [], skills: [], models: [], config: [] },
        evidenceCandidates: [],
        error: safeScanError(error),
      };
    }
    completed += 1;
    if (completed % 5 === 0 || completed === repositories.length) {
      await persist();
      console.log(JSON.stringify({ completed, total: repositories.length, repository: item.repository }));
    }
  }
}

await Promise.all(Array.from({ length: 4 }, () => worker()));
await persist();
const counts = results.reduce((summary, record) => {
  summary[record.status] = (summary[record.status] ?? 0) + 1;
  return summary;
}, {});
console.log(JSON.stringify({ output, repositories: repositories.length, ...counts }));
