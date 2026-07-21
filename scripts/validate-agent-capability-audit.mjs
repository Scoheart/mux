#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const auditRoot = resolve(root, "analysis/agent-capability-audit");

const readJson = async (path) => JSON.parse(await readFile(path, "utf8"));
const [agents, catalog, acp, verifiedText, aToMText, nToZText] = await Promise.all([
  readJson(resolve(root, "data/agents.json")),
  readJson(resolve(root, "data/agent-catalog.json")),
  readJson(resolve(auditRoot, "acp-registry-source-discovery.json")),
  readFile(resolve(auditRoot, "verified-agents-evidence.md"), "utf8"),
  readFile(resolve(auditRoot, "catalog-a-m-evidence.md"), "utf8"),
  readFile(resolve(auditRoot, "catalog-n-z-evidence.md"), "utf8"),
]);

const allIds = new Set([
  ...Object.keys(agents),
  ...Object.keys(catalog),
  ...acp.records.map((record) => record.suggestedMuxId),
]);

function lastJsonBlock(markdown, label) {
  const blocks = [...markdown.matchAll(/```json\n([\s\S]*?)\n```/g)];
  if (blocks.length === 0) throw new Error(`${label} has no JSON summary block`);
  try {
    return JSON.parse(blocks.at(-1)[1]);
  } catch (error) {
    throw new Error(`${label} has an invalid final JSON summary: ${error.message}`);
  }
}

function unique(values, label) {
  const seen = new Set();
  const duplicates = [];
  for (const value of values) {
    if (seen.has(value)) duplicates.push(value);
    seen.add(value);
  }
  if (duplicates.length > 0) {
    throw new Error(`${label} contains duplicate identities: ${[...new Set(duplicates)].join(", ")}`);
  }
  return seen;
}

function between(markdown, startMarker, endMarker, label) {
  const start = markdown.indexOf(startMarker);
  const end = markdown.indexOf(endMarker, start + startMarker.length);
  if (start < 0 || end < 0 || end <= start) {
    throw new Error(`${label} is missing its bounded evidence table`);
  }
  return markdown.slice(start, end);
}

const verifiedMainTable = between(
  verifiedText,
  "### MCP 与 Skills 注册值逐项复核",
  "### Skills 安装探针与兼容目录",
  "verified-agents-evidence.md",
);
const verifiedIds = unique(
  [...verifiedMainTable.matchAll(/^\| `([^`]+)` \|/gm)].map((match) => match[1]),
  "verified main evidence table",
);
const aToMSummary = lastJsonBlock(aToMText, "catalog-a-m-evidence.md");
const aToMIds = unique(
  [
    ...(aToMSummary.research_candidates ?? []),
    ...(aToMSummary.read_only ?? []),
    ...(aToMSummary.misclassified ?? []),
    ...(aToMSummary.duplicates ?? []),
  ],
  "A-M summary",
);
const nToZSummary = lastJsonBlock(nToZText, "catalog-n-z-evidence.md");
const nToZIds = unique(
  Object.values(nToZSummary.primary_status_members ?? {}).flat(),
  "N-Z summary",
);

const reportSets = [
  ["verified", verifiedIds],
  ["catalog-a-m", aToMIds],
  ["catalog-n-z", nToZIds],
];
const ownership = new Map();
const overlaps = [];
for (const [label, ids] of reportSets) {
  for (const id of ids) {
    if (!allIds.has(id)) throw new Error(`${label} contains an unknown identity: ${id}`);
    const previous = ownership.get(id);
    if (previous) overlaps.push(`${id} (${previous}, ${label})`);
    ownership.set(id, label);
  }
}
if (overlaps.length > 0) throw new Error(`identities occur in multiple report shards: ${overlaps.join(", ")}`);

const missing = [...allIds].filter((id) => !ownership.has(id)).sort();
if (missing.length > 0) throw new Error(`audit coverage is incomplete (${missing.length} missing): ${missing.join(", ")}`);

if (aToMSummary.identity_count !== aToMIds.size) {
  throw new Error(`A-M identity_count=${aToMSummary.identity_count} but summary contains ${aToMIds.size}`);
}
if (nToZSummary.scope?.identity_count !== nToZIds.size) {
  throw new Error(`N-Z identity_count=${nToZSummary.scope?.identity_count} but summary contains ${nToZIds.size}`);
}

console.log(
  JSON.stringify(
    {
      identities: allIds.size,
      covered: ownership.size,
      shards: Object.fromEntries(reportSets.map(([label, ids]) => [label, ids.size])),
      missing: 0,
      overlaps: 0,
    },
    null,
    2,
  ),
);
