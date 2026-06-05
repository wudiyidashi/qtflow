#!/usr/bin/env node
import childProcess from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..");
const NPM_ROOT = path.join(REPO_ROOT, "npm");

const PLATFORMS = [
  {
    platform: "win32-x64",
    packageName: "@xehxx/qtflow-cli-win32-x64",
  },
  {
    platform: "linux-x64",
    packageName: "@xehxx/qtflow-cli-linux-x64",
  },
  {
    platform: "darwin-x64",
    packageName: "@xehxx/qtflow-cli-darwin-x64",
  },
  {
    platform: "darwin-arm64",
    packageName: "@xehxx/qtflow-cli-darwin-arm64",
  },
];

const SEMVER_LIKE_PATTERN = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

function usage() {
  return [
    "Usage:",
    "  node scripts/release.mjs bump <version> [--commit] [--push]",
    "  node scripts/release.mjs check",
    "  node scripts/release.mjs publish <version> [--dist <dir>] [--otp <code>] [--dry-run]",
  ].join("\n");
}

function relative(filePath) {
  return path.relative(REPO_ROOT, filePath).replace(/\\/g, "/");
}

function parseArgs(argv) {
  const [command, ...rest] = argv;
  const options = {
    positional: [],
    commit: false,
    push: false,
    dist: undefined,
    otp: undefined,
    dryRun: false,
  };

  for (let index = 0; index < rest.length; index += 1) {
    const arg = rest[index];
    if (arg === "--commit") {
      options.commit = true;
      continue;
    }
    if (arg === "--push") {
      options.push = true;
      continue;
    }
    if (arg === "--dry-run") {
      options.dryRun = true;
      continue;
    }
    if (arg === "--dist") {
      index += 1;
      options.dist = rest[index];
      if (!options.dist) {
        throw new Error("--dist requires a directory");
      }
      continue;
    }
    if (arg === "--otp") {
      index += 1;
      options.otp = rest[index];
      if (!options.otp) {
        throw new Error("--otp requires a code");
      }
      continue;
    }
    if (arg.startsWith("--")) {
      throw new Error(`Unknown option: ${arg}`);
    }
    options.positional.push(arg);
  }

  return { command, options };
}

function assertSemverLike(version) {
  if (!SEMVER_LIKE_PATTERN.test(version)) {
    throw new Error(`Invalid version "${version}". Expected x.y.z or x.y.z-pre`);
  }
}

function readText(filePath) {
  return fs.readFileSync(filePath, "utf8");
}

function writeTextIfChanged(filePath, content, changes) {
  const oldContent = fs.existsSync(filePath) ? readText(filePath) : "";
  if (oldContent !== content) {
    fs.writeFileSync(filePath, content, "utf8");
    changes.add(filePath);
  }
}

function readJson(filePath) {
  return JSON.parse(readText(filePath));
}

function writeJson(filePath, data, changes) {
  writeTextIfChanged(filePath, `${JSON.stringify(data, null, 2)}\n`, changes);
}

function cargoTomlPath() {
  return path.join(REPO_ROOT, "Cargo.toml");
}

function cargoLockPath() {
  return path.join(REPO_ROOT, "Cargo.lock");
}

function mainPackagePath() {
  return path.join(NPM_ROOT, "qtflow", "package.json");
}

function platformPackagePath(platform) {
  return path.join(NPM_ROOT, "platforms", platform.platform, "package.json");
}

function readCargoVersion() {
  const cargoToml = readText(cargoTomlPath());
  const match = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error("Could not find package.version in Cargo.toml");
  }
  return match[1];
}

function readQtflowCargoLockVersion() {
  const lockPath = cargoLockPath();
  if (!fs.existsSync(lockPath)) {
    throw new Error("Cargo.lock is missing");
  }

  const cargoLock = readText(lockPath);
  const packageMatch = cargoLock.match(/(^\[\[package\]\]\r?\nname\s*=\s*"qtflow"\r?\nversion\s*=\s*"([^"]+)")/m);
  if (!packageMatch) {
    throw new Error("Could not find qtflow package entry in Cargo.lock");
  }
  return packageMatch[2];
}

function readVersionFields() {
  const fields = [
    {
      file: cargoTomlPath(),
      field: "package.version",
      value: readCargoVersion(),
    },
    {
      file: cargoLockPath(),
      field: "package qtflow.version",
      value: readQtflowCargoLockVersion(),
    },
  ];

  const mainPath = mainPackagePath();
  const mainPackage = readJson(mainPath);
  fields.push({
    file: mainPath,
    field: "version",
    value: mainPackage.version,
  });

  for (const platform of PLATFORMS) {
    fields.push({
      file: mainPath,
      field: `optionalDependencies.${platform.packageName}`,
      value: mainPackage.optionalDependencies?.[platform.packageName],
    });
  }

  for (const platform of PLATFORMS) {
    const packagePath = platformPackagePath(platform);
    const platformPackage = readJson(packagePath);
    fields.push({
      file: packagePath,
      field: "version",
      value: platformPackage.version,
    });
  }

  return fields;
}

function checkVersions(expectedVersion) {
  const fields = readVersionFields();
  const values = new Set(fields.map((field) => field.value));
  const missing = fields.filter((field) => !field.value);
  const drifted = values.size !== 1 || missing.length > 0;
  const mismatchedExpected = expectedVersion ? fields.filter((field) => field.value !== expectedVersion) : [];

  if (!drifted && mismatchedExpected.length === 0) {
    console.log(`Version check passed: ${fields[0].value}`);
    return fields[0].value;
  }

  console.error("Version check failed:");
  for (const field of fields) {
    const value = field.value === undefined ? "<missing>" : field.value;
    const expectedNote = expectedVersion && value !== expectedVersion ? ` (expected ${expectedVersion})` : "";
    console.error(`  ${relative(field.file)} ${field.field}: ${value}${expectedNote}`);
  }
  throw new ExitError(1);
}

function replaceCargoTomlVersion(version, changes) {
  const filePath = cargoTomlPath();
  const oldContent = readText(filePath);
  let replaced = false;
  const newContent = oldContent.replace(/^version\s*=\s*"([^"]+)"/m, (line) => {
    replaced = true;
    return line.replace(/"[^"]+"/, `"${version}"`);
  });

  if (!replaced) {
    throw new Error("Could not find package.version in Cargo.toml");
  }
  writeTextIfChanged(filePath, newContent, changes);
}

function replaceCargoLockVersion(version, changes) {
  const filePath = cargoLockPath();
  if (!fs.existsSync(filePath)) {
    return;
  }

  const oldContent = readText(filePath);
  let replaced = false;
  const newContent = oldContent.replace(
    /(^\[\[package\]\]\r?\nname\s*=\s*"qtflow"\r?\nversion\s*=\s*")([^"]+)(")/m,
    (match, prefix, oldVersion, suffix) => {
      replaced = true;
      return `${prefix}${version}${suffix}`;
    },
  );

  if (!replaced) {
    throw new Error("Could not find qtflow package entry in Cargo.lock");
  }
  writeTextIfChanged(filePath, newContent, changes);
}

function updatePackageJsonVersions(version, changes) {
  const mainPath = mainPackagePath();
  const mainPackage = readJson(mainPath);
  mainPackage.version = version;
  if (!mainPackage.optionalDependencies) {
    mainPackage.optionalDependencies = {};
  }
  for (const platform of PLATFORMS) {
    mainPackage.optionalDependencies[platform.packageName] = version;
  }
  writeJson(mainPath, mainPackage, changes);

  for (const platform of PLATFORMS) {
    const packagePath = platformPackagePath(platform);
    const platformPackage = readJson(packagePath);
    platformPackage.version = version;
    writeJson(packagePath, platformPackage, changes);
  }
}

function commandExists(command) {
  const probe = process.platform === "win32" ? "where" : "command";
  const args = process.platform === "win32" ? [command] : ["-v", command];
  const result = childProcess.spawnSync(probe, args, {
    cwd: REPO_ROOT,
    stdio: "ignore",
    shell: process.platform !== "win32",
  });
  return result.status === 0;
}

function runCommand(command, args, options = {}) {
  const printable = [command, ...redactArgs(args)].join(" ");
  console.log(`$ ${printable}`);
  const result = childProcess.spawnSync(command, args, {
    cwd: REPO_ROOT,
    stdio: "inherit",
    shell: process.platform === "win32" && command === "npm",
    ...options,
  });

  if (result.error) {
    throw new Error(`${printable} failed: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`${printable} failed with exit code ${result.status}`);
  }
}

function redactArgs(args) {
  const redacted = [...args];
  for (let index = 0; index < redacted.length; index += 1) {
    if (redacted[index] === "--otp" && index + 1 < redacted.length) {
      redacted[index + 1] = "<redacted>";
      index += 1;
    }
  }
  return redacted;
}

function runCargoUpdate(changes, version) {
  if (!commandExists("cargo")) {
    console.warn("Warning: cargo is not available; Cargo.lock was updated directly but cargo update -p qtflow was skipped.");
    replaceCargoLockVersion(version, changes);
    return;
  }

  runCommand("cargo", ["update", "-p", "qtflow"]);
  replaceCargoLockVersion(version, changes);
}

function printBumpSummary(beforeFields, afterFields, changes) {
  const beforeByKey = new Map(beforeFields.map((field) => [`${field.file}:${field.field}`, field.value]));
  console.log("");
  console.log("Version summary:");
  for (const field of afterFields) {
    const oldVersion = beforeByKey.get(`${field.file}:${field.field}`);
    console.log(`  ${relative(field.file)} ${field.field}: ${oldVersion} -> ${field.value}`);
  }

  console.log("");
  if (changes.size === 0) {
    console.log("No files changed.");
    return;
  }

  console.log("Files changed:");
  for (const filePath of [...changes].sort()) {
    console.log(`  ${relative(filePath)}`);
  }
}

function runGitRelease(version, push) {
  runCommand("git", ["add", "-A"]);
  runCommand("git", ["commit", "-m", `release: v${version}`]);
  runCommand("git", ["tag", `v${version}`]);
  if (push) {
    runCommand("git", ["push", "--follow-tags"]);
  }
}

function nextSteps(version, committed, pushed) {
  console.log("");
  if (committed) {
    if (pushed) {
      console.log(`Pushed release commit and tag v${version}. Wait for the GitHub Release workflow, then publish npm packages.`);
    } else {
      console.log(`Created release commit and tag v${version}. Next: git push --follow-tags`);
    }
    return;
  }

  console.log("Next steps:");
  console.log("  1. Review the version changes.");
  console.log(`  2. Commit and tag: git add -A && git commit -m "release: v${version}" && git tag v${version}`);
  console.log("  3. Push the tag: git push --follow-tags");
  console.log(`  4. After the GitHub Release workflow finishes, publish: node scripts/release.mjs publish ${version}`);
}

function bump(version, options) {
  assertSemverLike(version);

  if (options.push && !options.commit) {
    throw new Error("--push requires --commit");
  }

  const beforeFields = readVersionFields();
  const changes = new Set();

  replaceCargoTomlVersion(version, changes);
  updatePackageJsonVersions(version, changes);
  runCargoUpdate(changes, version);

  const afterFields = readVersionFields();
  printBumpSummary(beforeFields, afterFields, changes);

  if (options.commit) {
    runGitRelease(version, options.push);
  }
  nextSteps(version, options.commit, options.push);
}

function createNpmEnv() {
  const token = process.env.NODE_AUTH_TOKEN || process.env.NPM_TOKEN;
  if (!token) {
    return {
      env: { ...process.env },
      cleanup: () => {},
    };
  }

  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "qtflow-npm-"));
  const userconfig = path.join(tempDir, ".npmrc");
  fs.writeFileSync(userconfig, `//registry.npmjs.org/:_authToken=${token}\n`, { mode: 0o600 });
  const env = { ...process.env };
  delete env.NODE_AUTH_TOKEN;
  delete env.NPM_TOKEN;
  env.npm_config_userconfig = userconfig;

  return {
    env,
    cleanup: () => {
      fs.rmSync(tempDir, { recursive: true, force: true });
    },
  };
}

function ghReleaseDownload(version, distDir) {
  fs.mkdirSync(distDir, { recursive: true });
  runCommand("gh", ["release", "download", `v${version}`, "--dir", distDir]);
}

function populateBinaries(version, distDir) {
  runCommand(process.execPath, [
    path.join(NPM_ROOT, "scripts", "populate-binaries.mjs"),
    "--version",
    version,
    "--dist",
    distDir,
  ]);
}

function publishPackage(packageDir, options, npmEnv) {
  const args = ["publish", packageDir, "--access", "public"];
  if (options.otp) {
    args.push("--otp", options.otp);
  }
  if (options.dryRun) {
    args.push("--dry-run");
  }

  runCommand("npm", args, {
    env: npmEnv.env,
  });
  console.log(`${options.dryRun ? "Dry-run published" : "Published"} ${relative(packageDir)}`);
}

function publish(version, options) {
  assertSemverLike(version);
  checkVersions(version);

  const distDir = path.resolve(REPO_ROOT, options.dist ?? "dist");
  ghReleaseDownload(version, distDir);
  populateBinaries(version, distDir);

  const npmEnv = createNpmEnv();
  try {
    for (const platform of PLATFORMS) {
      publishPackage(platformPackagePath(platform), options, npmEnv);
    }
    publishPackage(mainPackagePath(), options, npmEnv);
  } finally {
    npmEnv.cleanup();
  }
}

class ExitError extends Error {
  constructor(code) {
    super(`Exit ${code}`);
    this.code = code;
  }
}

function main() {
  const { command, options } = parseArgs(process.argv.slice(2));

  if (command === "--help" || command === "-h" || !command) {
    console.log(usage());
    return;
  }

  if (command === "check") {
    if (options.positional.length > 0) {
      throw new Error("check does not accept positional arguments");
    }
    checkVersions();
    return;
  }

  if (command === "bump") {
    const [version, ...extra] = options.positional;
    if (!version || extra.length > 0) {
      throw new Error("bump requires exactly one <version>");
    }
    bump(version, options);
    return;
  }

  if (command === "publish") {
    const [version, ...extra] = options.positional;
    if (!version || extra.length > 0) {
      throw new Error("publish requires exactly one <version>");
    }
    publish(version, options);
    return;
  }

  throw new Error(`Unknown command: ${command}`);
}

try {
  main();
} catch (error) {
  if (error instanceof ExitError) {
    process.exit(error.code);
  }
  console.error(error.message);
  console.error("");
  console.error(usage());
  process.exit(1);
}
