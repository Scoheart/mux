import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

async function read(relativePath) {
  return readFile(join(root, relativePath), "utf8");
}

function jobBlock(workflow, job, nextJob) {
  const start = workflow.indexOf(`\n  ${job}:`);
  assert.notEqual(start, -1, `missing ${job} job`);
  const end = nextJob ? workflow.indexOf(`\n  ${nextJob}:`, start + 1) : -1;
  return workflow.slice(start, end === -1 ? undefined : end);
}

test("quality workflow runs independent producer jobs", async () => {
  const workflow = await read(".github/workflows/quality-monitor.yml");
  const rust = jobBlock(workflow, "rust", "desktop");
  const desktop = jobBlock(workflow, "desktop", "website");
  const website = jobBlock(workflow, "website", "verify");

  assert.match(rust, /cargo test --locked -p mux-core -p mux-cli/);
  assert.match(desktop, /node-version:\s*24/);
  assert.match(desktop, /cache:\s*npm/);
  assert.match(desktop, /cache-dependency-path:\s*desktop\/package-lock\.json/);
  assert.match(desktop, /npm ci --no-audit --no-fund/);
  assert.match(desktop, /node scripts\/release-version\.mjs check/);
  assert.match(website, /node-version:\s*24/);
  assert.match(website, /cache-dependency-path:\s*website\/package-lock\.json/);
  assert.match(website, /npm ci --no-audit --no-fund/);
  assert.doesNotMatch(workflow, /npm install/);
});

test("verify is the stable aggregate result", async () => {
  const workflow = await read(".github/workflows/quality-monitor.yml");
  const verify = jobBlock(workflow, "verify", "monitor");

  assert.match(verify, /name:\s*verify/);
  assert.match(verify, /if:\s*\$\{\{ always\(\) \}\}/);
  assert.match(verify, /needs:\s*\[rust, desktop, website\]/);
  for (const producer of ["rust", "desktop", "website"]) {
    assert.match(verify, new RegExp(`needs\\.${producer}\\.result`));
  }
});

test("monitor owns the non-PR failure lifecycle", async () => {
  const workflow = await read(".github/workflows/quality-monitor.yml");
  const monitor = jobBlock(workflow, "monitor");

  assert.match(monitor, /if:\s*\$\{\{ always\(\) \}\}/);
  assert.match(monitor, /needs:\s*\[verify\]/);
  assert.match(monitor, /needs\.verify\.result == 'failure'/);
  assert.match(monitor, /github\.event_name != 'pull_request'/);
  assert.match(monitor, /needs\.verify\.result == 'success'/);
  assert.match(monitor, /secrets\.COPILOT_PAT/);
  assert.match(workflow, /cancel-in-progress:\s*true/);
});
