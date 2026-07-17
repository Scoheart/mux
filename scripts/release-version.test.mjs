import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  collectVersionMismatches,
  isReleaseMerge,
} from "./release-version.mjs";

async function writeJson(path, value) {
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`);
}

async function createFixture(overrides = {}) {
  const root = await mkdtemp(join(tmpdir(), "mux-release-version-"));
  await mkdir(join(root, "core"), { recursive: true });
  await mkdir(join(root, "cli"), { recursive: true });
  await mkdir(join(root, "desktop", "src-tauri"), { recursive: true });

  const version = overrides.version ?? "1.2.18";
  await writeFile(join(root, "version.txt"), `${version}\n`);
  await writeFile(
    join(root, "core", "Cargo.toml"),
    `[package]\nname = "mux-core"\nversion = "${overrides.core ?? version}"\n`,
  );
  await writeFile(
    join(root, "cli", "Cargo.toml"),
    `[package]\nname = "mux-cli"\nversion = "${overrides.cli ?? version}"\n`,
  );
  await writeJson(join(root, "desktop", "package.json"), {
    name: "desktop",
    version: overrides.packageJson ?? version,
  });
  await writeJson(join(root, "desktop", "package-lock.json"), {
    name: "desktop",
    version: overrides.lockRoot ?? version,
    lockfileVersion: 3,
    packages: {
      "": {
        name: "desktop",
        version: overrides.lockPackage ?? version,
      },
      ...overrides.lockPackages,
    },
  });
  await writeFile(
    join(root, "desktop", "src-tauri", "Cargo.toml"),
    `[package]\nname = "desktop"\nversion = "${overrides.tauriCargo ?? version}"\n`,
  );
  await writeJson(join(root, "desktop", "src-tauri", "tauri.conf.json"), {
    productName: "MUX",
    version: overrides.tauriConfig ?? version,
  });

  return root;
}

test("all release-owned version fields agree", async () => {
  const root = await createFixture();
  assert.deepEqual(await collectVersionMismatches(root), []);
});

test("reports both npm lockfile fields independently", async () => {
  const root = await createFixture({
    lockRoot: "1.2.16",
    lockPackage: "1.2.17",
  });
  const mismatches = await collectVersionMismatches(root);

  assert.ok(
    mismatches.some((message) =>
      message.includes("desktop/package-lock.json.version"),
    ),
  );
  assert.ok(
    mismatches.some((message) =>
      message.includes('desktop/package-lock.json.packages[""].version'),
    ),
  );
});

test("reports transitive npm packages missing from a portable lockfile", async () => {
  const root = await createFixture({
    lockPackages: {
      "node_modules/bundler": {
        version: "1.0.0",
        optionalDependencies: {
          "bundler-linux-x64": "1.0.0",
        },
      },
      "node_modules/bundler-linux-x64": {
        version: "1.0.0",
        optional: true,
        dependencies: {
          "bundler-runtime": "1.0.0",
        },
      },
    },
  });
  const mismatches = await collectVersionMismatches(root);

  assert.ok(
    mismatches.some(
      (message) =>
        message.includes("bundler-runtime") &&
        message.includes("node_modules/bundler-linux-x64"),
    ),
  );
});

test("reports the manifest path when a Cargo package differs", async () => {
  const root = await createFixture({ cli: "1.2.17" });
  const mismatches = await collectVersionMismatches(root);

  assert.ok(
    mismatches.some(
      (message) =>
        message.includes("cli/Cargo.toml") && message.includes("1.2.17"),
    ),
  );
});

test("rejects a stable tag that does not match the version source", async () => {
  const root = await createFixture();
  const mismatches = await collectVersionMismatches(root, {
    stableTag: "v1.2.17",
  });

  assert.ok(mismatches.some((message) => message.includes("stable tag")));
});

test("classifies a Release Please merge only when both signals agree", () => {
  assert.equal(
    isReleaseMerge("1.2.18", "1.3.0", "chore(main): release 1.3.0"),
    true,
  );
  assert.equal(
    isReleaseMerge("1.2.18", "1.3.0", "feat: ship a feature"),
    false,
  );
  assert.equal(
    isReleaseMerge("1.3.0", "1.3.0", "chore(main): release 1.3.0"),
    false,
  );
});

test("rejects malformed version.txt values", async () => {
  const root = await createFixture({ version: "v1.2.18" });
  const mismatches = await collectVersionMismatches(root);

  assert.ok(mismatches.some((message) => message.includes("version.txt")));
});
