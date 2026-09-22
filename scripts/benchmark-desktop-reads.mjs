// Pure read-path benchmarks with synthetic data; no desktop IPC, network or keys.
// node --experimental-vm-modules scripts/benchmark-desktop-reads.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { stripTypeScriptTypes } from "node:module";
import vm from "node:vm";

const root = new URL("../", import.meta.url);
const baselineArgument = process.argv.indexOf("--baseline");
const baseline = baselineArgument < 0 ? "HEAD" : process.argv[baselineArgument + 1];
assert(baseline, "--baseline requires a Git revision");
function source(path, revision) {
  return revision ? execFileSync("git", ["show", `${revision}:${path}`], { cwd: root, encoding: "utf8" })
    : readFileSync(new URL(path, root), "utf8");
}
async function pureModule(path, revision) {
  const text = stripTypeScriptTypes(source(path, revision));
  return import(`data:text/javascript;base64,${Buffer.from(text).toString("base64")}#${revision ?? "current"}`);
}
function measure(name, work) {
  work();
  const samples = [];
  for (let run = 0; run < 21; run++) {
    const start = performance.now();
    work();
    samples.push(performance.now() - start);
  }
  samples.sort((a, b) => a - b);
  console.log(JSON.stringify({ scenario: name, median_ms: samples[10], p95_ms: samples[19] }));
}
const before = await pureModule("desktop/src/lib/consumption.ts", baseline);
const after = await pureModule("desktop/src/lib/consumption.ts");
const assets = Array.from({ length: 300 }, (_, i) => ({ domain: "mcp", key: `server-${i}::stdio` }));
const rows = Array.from({ length: 6000 }, (_, i) => ({
  asset: assets[i % assets.length], agent_id: `agent-${Math.floor(i / assets.length)}`,
  observed: i % 7 !== 0, enabled: i % 11 !== 0,
}));
// Include colliding IDs in another domain and duplicate external observations.
const inventory = { consumptions: [...rows, { ...rows[1], asset: { domain: "skill", name: assets[1].key }, agent_id: "wrong-domain" }], external: rows.slice(0, 400) };
const index = after.observedAgentIndex(inventory, "mcp");
for (const asset of assets) assert.deepEqual(index.get(asset.key) ?? [], before.observedAgentIdsForAsset(inventory, asset));
measure("relationships_300_cards_before", () => assets.map((asset) => before.observedAgentIdsForAsset(inventory, asset)));
measure("relationships_300_cards_after", () => {
  const index = after.observedAgentIndex(inventory, "mcp");
  return assets.map((asset) => index.get(asset.key) ?? []);
});

const catalog = { fixture: { models: Object.fromEntries(Array.from({ length: 20 }, (_, i) => [`model-${i}`, { name: `Model ${i}`, limit: { context: 128000 } }])) } };
// Timer eviction should not keep a CLI benchmark alive for five minutes.
const nativeTimeout = globalThis.setTimeout;
globalThis.setTimeout = (...args) => { const handle = nativeTimeout(...args); handle.unref(); return handle; };
for (const revision of [baseline, undefined]) {
  const metadata = await pureModule("desktop/src/lib/modelsDev.ts", revision);
  let stored = null;
  let requests = 0;
  const storage = { getItem: () => stored, setItem: (_, value) => { stored = value; } };
  const fetchImpl = async () => {
    requests++;
    await Promise.resolve();
    return { ok: true, json: async () => catalog };
  };
  const profiles = Array.from({ length: 20 }, (_, i) => ({ id: `profile-${i}`, provider: "fixture", model: `model-${i}`, catalog_key: `fixture/model-${i}`, base_url: "https://example.invalid/v1" }));
  const results = await Promise.all(profiles.map((profile) => metadata.loadModelsDevMetadata([profile], { storage, fetchImpl, now: () => 1000 })));
  assert(results.every((result, i) => result[`profile-${i}`]?.contextWindow === 128000));
  const retained = Object.keys(JSON.parse(stored).entries).length;
  if (!revision) { assert.equal(requests, 1); assert.equal(retained, 20); }
  console.log(JSON.stringify({ scenario: `metadata_concurrent_${revision ? "before" : "after"}`, callers: 20, requests, retained_profiles: retained }));
}
globalThis.setTimeout = nativeTimeout;

// Exercise navigation reuse, invalidation and late-response exclusion without
// invoking the real Tauri bridge or retaining any credential material.
let revision = 0;
let calls = 0;
let delay = null;
let fail = false;
const api = new vm.SyntheticModule(["listModelProfiles", "listModelProviders", "listModelProviderInstances"], function () {
  for (const name of ["listModelProfiles", "listModelProviders", "listModelProviderInstances"]) {
    this.setExport(name, async () => { calls++; if (delay) await delay; if (fail) throw new Error("fixture"); return []; });
  }
});
const clock = new vm.SyntheticModule(["modelObservationRevision"], function () { this.setExport("modelObservationRevision", () => revision); });
const library = new vm.SourceTextModule(stripTypeScriptTypes(source("desktop/src/lib/modelLibrary.ts")));
await library.link((name) => name === "./api" ? api : clock);
await library.evaluate();
const { loadModelLibrary, cachedModelLibrary } = library.namespace;
for (let i = 0; i < 10; i++) await loadModelLibrary();
assert.equal(calls, 3);
console.log(JSON.stringify({ scenario: "model_navigation_10_visits", before_ipc_reads: 30, after_ipc_reads: calls }));
revision++;
await Promise.all([loadModelLibrary(), loadModelLibrary()]);
assert.equal(calls, 6);
await loadModelLibrary(true);
assert.equal(calls, 9);
fail = true;
await assert.rejects(loadModelLibrary(true));
assert.equal(cachedModelLibrary(), null);
fail = false;
revision++;
let release;
delay = new Promise((resolve) => { release = resolve; });
const late = loadModelLibrary();
revision++;
release(); await late;
assert.equal(cachedModelLibrary(), null);
delay = null; fail = true;
await assert.rejects(loadModelLibrary());
fail = false;
await loadModelLibrary();
assert(cachedModelLibrary());
console.log(JSON.stringify({ scenario: "read_cache_invalidation_and_recovery", result: "pass" }));
