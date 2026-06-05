#!/usr/bin/env node
"use strict";

const { spawnSync } = require("child_process");
const path = require("path");

const PACKAGES = {
  "win32:x64": {
    packageName: "@xehxx/qtflow-cli-win32-x64",
    binary: "qtflow.exe",
  },
  "linux:x64": {
    packageName: "@xehxx/qtflow-cli-linux-x64",
    binary: "qtflow",
  },
  "darwin:x64": {
    packageName: "@xehxx/qtflow-cli-darwin-x64",
    binary: "qtflow",
  },
  "darwin:arm64": {
    packageName: "@xehxx/qtflow-cli-darwin-arm64",
    binary: "qtflow",
  },
};

function installHelp(platform, arch) {
  return [
    `qtflow: no prebuilt npm package is installed for ${platform}-${arch}.`,
    "",
    "Install from a GitHub release archive, or build from source with:",
    "  cargo install qtflow",
    "",
    "GitHub releases: https://github.com/wudiyidashi/qtflow/releases",
  ].join("\n");
}

function resolveBinary(platform = process.platform, arch = process.arch, requireResolve = require.resolve) {
  const entry = PACKAGES[`${platform}:${arch}`];
  if (!entry) {
    return {
      ok: false,
      message: installHelp(platform, arch),
    };
  }

  try {
    const packageJson = requireResolve(`${entry.packageName}/package.json`);
    const packageRoot = path.dirname(packageJson);
    return {
      ok: true,
      packageName: entry.packageName,
      binaryPath: path.join(packageRoot, "bin", entry.binary),
    };
  } catch (error) {
    return {
      ok: false,
      packageName: entry.packageName,
      message: [
        `qtflow: optional package ${entry.packageName} is not installed or cannot be resolved.`,
        "",
        "This can happen when optional dependencies are disabled or this platform is unsupported.",
        "Install from a GitHub release archive, or build from source with:",
        "  cargo install qtflow",
        "",
        "GitHub releases: https://github.com/wudiyidashi/qtflow/releases",
      ].join("\n"),
      cause: error,
    };
  }
}

function run(argv = process.argv.slice(2)) {
  const resolved = resolveBinary();
  if (!resolved.ok) {
    console.error(resolved.message);
    return 1;
  }

  const child = spawnSync(resolved.binaryPath, argv, {
    stdio: "inherit",
  });

  if (child.error) {
    console.error(`qtflow: failed to launch ${resolved.binaryPath}: ${child.error.message}`);
    return 1;
  }

  if (child.signal) {
    console.error(`qtflow: process terminated by signal ${child.signal}`);
    return 1;
  }

  return child.status === null ? 1 : child.status;
}

if (require.main === module) {
  process.exit(run());
}

module.exports = {
  PACKAGES,
  resolveBinary,
  run,
};
