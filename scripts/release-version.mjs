#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const SEMVER_PATTERN = /^\d+\.\d+\.\d+$/;
const STABLE_TAG_PATTERN = /^v(\d+\.\d+\.\d+)$/;
const REPOSITORY_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const SOURCE_FIELDS = [
  ["core/Cargo.toml", "cargo"],
  ["cli/Cargo.toml", "cargo"],
  ["desktop/package.json", "json", ["version"]],
  ["desktop/src-tauri/Cargo.toml", "cargo"],
  ["desktop/src-tauri/tauri.conf.json", "json", ["version"]],
];

const GENERATED_FIELDS = [
  ["desktop/package-lock.json", "json", ["version"]],
  ["desktop/package-lock.json", "json", ["packages", "", "version"]],
];

function displayPath(root, path) {
  return relative(root, path).replaceAll("\\", "/");
}

function formatJsonField(path) {
  return path
    .map((part, index) => {
      if (part === "") return '[""]';
      return index === 0 ? part : `.${part}`;
    })
    .join("");
}

function jsonValueAt(value, path) {
  let current = value;
  for (const part of path) {
    if (
      current === null ||
      typeof current !== "object" ||
      !Object.hasOwn(current, part)
    ) {
      return undefined;
    }
    current = current[part];
  }
  return current;
}

export function readCargoPackageVersion(contents, path = "Cargo.toml") {
  let inPackage = false;
  const versions = [];

  for (const line of contents.split(/\r?\n/)) {
    const section = line.match(/^\s*\[([^\]]+)\]\s*(?:#.*)?$/);
    if (section) {
      inPackage = section[1] === "package";
      continue;
    }
    if (!inPackage) continue;

    const version = line.match(
      /^\s*version\s*=\s*"([^"]+)"\s*(?:#.*)?$/,
    );
    if (version) versions.push(version[1]);
  }

  if (versions.length !== 1) {
    throw new Error(
      `${path} must contain exactly one [package] version; found ${versions.length}`,
    );
  }
  return versions[0];
}

async function readExpectedVersion(root, mismatches) {
  const path = join(root, "version.txt");
  try {
    const version = (await readFile(path, "utf8")).trim();
    if (!SEMVER_PATTERN.test(version)) {
      mismatches.push(
        `version.txt must contain MAJOR.MINOR.PATCH, received ${JSON.stringify(version)}`,
      );
      return null;
    }
    return version;
  } catch (error) {
    mismatches.push(`version.txt cannot be read: ${error.message}`);
    return null;
  }
}

async function inspectField(root, definition, expected, mismatches) {
  const [relativePath, kind, fieldPath] = definition;
  const path = join(root, relativePath);
  let actual;

  try {
    const contents = await readFile(path, "utf8");
    if (kind === "cargo") {
      actual = readCargoPackageVersion(contents, relativePath);
    } else {
      const value = JSON.parse(contents);
      actual = jsonValueAt(value, fieldPath);
      if (typeof actual !== "string") {
        throw new Error(
          `${relativePath}.${formatJsonField(fieldPath)} must be a string`,
        );
      }
    }
  } catch (error) {
    mismatches.push(`${displayPath(root, path)}: ${error.message}`);
    return;
  }

  if (actual !== expected) {
    const field =
      kind === "cargo"
        ? relativePath
        : `${relativePath}.${formatJsonField(fieldPath)}`;
    mismatches.push(`${field} is ${actual}; expected ${expected}`);
  }
}

export async function collectVersionMismatches(
  root = REPOSITORY_ROOT,
  { includeGenerated = true, stableTag } = {},
) {
  const mismatches = [];
  const expected = await readExpectedVersion(root, mismatches);
  if (!expected) return mismatches;

  const definitions = includeGenerated
    ? [...SOURCE_FIELDS, ...GENERATED_FIELDS]
    : SOURCE_FIELDS;
  await Promise.all(
    definitions.map((definition) =>
      inspectField(root, definition, expected, mismatches),
    ),
  );

  if (stableTag !== undefined) {
    const match = stableTag.match(STABLE_TAG_PATTERN);
    if (!match) {
      mismatches.push(
        `stable tag must match vMAJOR.MINOR.PATCH, received ${JSON.stringify(stableTag)}`,
      );
    } else if (match[1] !== expected) {
      mismatches.push(`stable tag ${stableTag} does not match version ${expected}`);
    }
  }

  return mismatches;
}

export function isReleaseMerge(beforeVersion, afterVersion, commitTitle) {
  return (
    SEMVER_PATTERN.test(beforeVersion) &&
    SEMVER_PATTERN.test(afterVersion) &&
    beforeVersion !== afterVersion &&
    commitTitle === `chore(main): release ${afterVersion}`
  );
}

function run(command, args, cwd, stdio = "inherit") {
  execFileSync(command, args, { cwd, stdio });
}

function cargoLockErrors(root) {
  const commands = [
    ["cargo", ["metadata", "--locked", "--no-deps", "--format-version", "1"]],
    [
      "cargo",
      [
        "metadata",
        "--locked",
        "--no-deps",
        "--format-version",
        "1",
        "--manifest-path",
        "desktop/src-tauri/Cargo.toml",
      ],
    ],
  ];
  const errors = [];

  for (const [command, args] of commands) {
    try {
      run(command, args, root, "pipe");
    } catch (error) {
      const details = error.stderr?.toString().trim() || error.message;
      errors.push(`${command} ${args.join(" ")} failed: ${details}`);
    }
  }
  return errors;
}

async function failOnMismatches(mismatches) {
  if (mismatches.length === 0) return;
  for (const mismatch of mismatches) console.error(`- ${mismatch}`);
  throw new Error(`release version check failed with ${mismatches.length} error(s)`);
}

async function check(root, stableTag) {
  const mismatches = await collectVersionMismatches(root, { stableTag });
  mismatches.push(...cargoLockErrors(root));
  await failOnMismatches(mismatches);
  console.log("Release version metadata is consistent.");
}

async function refreshLocks(root) {
  await failOnMismatches(
    await collectVersionMismatches(root, { includeGenerated: false }),
  );
  run(
    "npm",
    [
      "install",
      "--package-lock-only",
      "--ignore-scripts",
      "--no-audit",
      "--no-fund",
      "--prefix",
      "desktop",
    ],
    root,
  );
  run("cargo", ["metadata", "--no-deps", "--format-version", "1"], root, "ignore");
  run(
    "cargo",
    [
      "metadata",
      "--no-deps",
      "--format-version",
      "1",
      "--manifest-path",
      "desktop/src-tauri/Cargo.toml",
    ],
    root,
    "ignore",
  );
  await check(root);
}

function parseArguments(argv) {
  const [command, ...args] = argv;
  let root = REPOSITORY_ROOT;
  let stableTag;

  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === "--root" && args[index + 1]) {
      root = resolve(args[++index]);
    } else if (args[index] === "--tag" && args[index + 1]) {
      stableTag = args[++index];
    } else {
      throw new Error(`unknown argument: ${args[index]}`);
    }
  }
  return { command, root, stableTag };
}

async function main() {
  const { command, root, stableTag } = parseArguments(process.argv.slice(2));
  if (command === "check") {
    await check(root, stableTag);
  } else if (command === "refresh-locks") {
    if (stableTag) throw new Error("refresh-locks does not accept --tag");
    await refreshLocks(root);
  } else {
    throw new Error(
      "usage: node scripts/release-version.mjs <check|refresh-locks> [--tag vX.Y.Z] [--root PATH]",
    );
  }
}

const isEntrypoint =
  process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url;
if (isEntrypoint) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
