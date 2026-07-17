import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
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

test("Release Please is one root component with generated locks", async () => {
  const config = JSON.parse(await read("release-please-config.json"));
  const manifest = JSON.parse(await read(".release-please-manifest.json"));

  assert.deepEqual(Object.keys(config.packages), ["."]);
  assert.equal(config["release-type"], "simple");
  assert.equal(config["include-component-in-tag"], false);
  assert.equal(config["include-v-in-tag"], true);
  assert.equal(config["separate-pull-requests"], false);
  assert.equal(config.draft, true);
  assert.equal(config["force-tag-creation"], true);
  assert.equal(config["always-update"], true);
  assert.deepEqual(manifest, { ".": "1.2.18" });

  const extraFiles = config.packages["."]["extra-files"];
  const paths = extraFiles.map((entry) => entry.path);
  for (const path of [
    "core/Cargo.toml",
    "cli/Cargo.toml",
    "desktop/package.json",
    "desktop/src-tauri/Cargo.toml",
    "desktop/src-tauri/tauri.conf.json",
  ]) {
    assert.ok(paths.includes(path), `missing release extra-file ${path}`);
  }
  assert.ok(paths.every((path) => !path.endsWith("lock.json")));
  assert.ok(paths.every((path) => !path.endsWith("Cargo.lock")));
});

test("Release Please uses the dedicated token and refreshes its PR", async () => {
  const workflow = await read(".github/workflows/release-please.yml");

  assert.match(workflow, /branches:\s*\[main\]/);
  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /RELEASE_PLEASE_TOKEN:\s*\$\{\{ secrets\.RELEASE_PLEASE_TOKEN \}\}/);
  assert.doesNotMatch(workflow, /token:\s*\$\{\{ github\.token \}\}/);
  assert.match(workflow, /outputs\.prs_created == 'true'/);
  assert.match(workflow, /fromJSON\(steps\.release\.outputs\.pr\)\.headBranchName/);
  assert.match(workflow, /node scripts\/release-version\.mjs refresh-locks/);
  assert.match(workflow, /desktop\/package-lock\.json Cargo\.lock desktop\/src-tauri\/Cargo\.lock/);

  const actionReferences = [...workflow.matchAll(/uses:\s*[^@\s]+@([^\s#]+)/g)];
  assert.ok(actionReferences.length > 0);
  for (const [, reference] of actionReferences) {
    assert.match(reference, /^[0-9a-f]{40}$/);
  }
});

test("desktop workflow classifies and gates both publication channels", async () => {
  const workflow = await read(".github/workflows/build-desktop.yml");

  assert.match(workflow, /node-version:\s*24/);
  assert.match(workflow, /cache-dependency-path:\s*desktop\/package-lock\.json/);
  assert.match(workflow, /npm ci --no-audit --no-fund/);
  assert.match(workflow, /\^v\[0-9\]\+\\\.\[0-9\]\+\\\.\[0-9\]\+\$/);
  assert.match(workflow, /chore\(main\): release/);
  assert.match(workflow, /wait-for-verify\.sh/);
  assert.match(workflow, /publish-release-assets\.sh/);
  assert.match(workflow, /gh release create/);
  assert.doesNotMatch(workflow, /cancel-in-progress:\s*true/);

  const prerelease = workflow.match(/# PRE-RELEASE START([\s\S]*?)# PRE-RELEASE END/);
  assert.ok(prerelease, "missing bounded Pre-release section");
  assert.doesNotMatch(prerelease[1], /latest\.json/);
  assert.doesNotMatch(prerelease[1], /publish-release-assets\.sh/);

  const stable = workflow.match(/# STABLE START([\s\S]*?)# STABLE END/);
  assert.ok(stable, "missing bounded Stable section");
  assert.match(stable[1], /publish-release-assets\.sh/);
  assert.doesNotMatch(stable[1], /gh release create/);
});

test("every repository Action uses an immutable commit", async () => {
  const workflowDirectory = join(root, ".github", "workflows");
  const workflowNames = (await readdir(workflowDirectory)).filter((name) =>
    name.endsWith(".yml"),
  );

  for (const workflowName of workflowNames) {
    const workflow = await read(`.github/workflows/${workflowName}`);
    for (const match of workflow.matchAll(/^\s*uses:\s*([^\s#]+).*$/gm)) {
      const action = match[1];
      if (action.startsWith("./")) continue;
      if (action.startsWith("docker://")) {
        assert.match(action, /@sha256:[0-9a-f]{64}$/);
        continue;
      }
      const separator = action.lastIndexOf("@");
      assert.notEqual(separator, -1, `${workflowName}: ${action}`);
      assert.match(
        action.slice(separator + 1),
        /^[0-9a-f]{40}$/,
        `${workflowName}: ${action}`,
      );
    }
  }
});
