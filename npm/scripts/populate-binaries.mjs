#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import zlib from "node:zlib";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const NPM_ROOT = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(NPM_ROOT, "..");

const PLATFORMS = [
  {
    platform: "win32-x64",
    packageName: "@qtflow/cli-win32-x64",
    target: "x86_64-pc-windows-msvc",
    archiveExt: "zip",
    archiveBinary: "qtflow.exe",
    packageBinary: "qtflow.exe",
  },
  {
    platform: "linux-x64",
    packageName: "@qtflow/cli-linux-x64",
    target: "x86_64-unknown-linux-gnu",
    archiveExt: "tar.gz",
    archiveBinary: "qtflow",
    packageBinary: "qtflow",
  },
  {
    platform: "darwin-x64",
    packageName: "@qtflow/cli-darwin-x64",
    target: "x86_64-apple-darwin",
    archiveExt: "tar.gz",
    archiveBinary: "qtflow",
    packageBinary: "qtflow",
  },
  {
    platform: "darwin-arm64",
    packageName: "@qtflow/cli-darwin-arm64",
    target: "aarch64-apple-darwin",
    archiveExt: "tar.gz",
    archiveBinary: "qtflow",
    packageBinary: "qtflow",
  },
];

function usage() {
  return [
    "Usage:",
    "  node npm/scripts/populate-binaries.mjs --version <version> --dist <release-asset-dir>",
    "  node npm/scripts/populate-binaries.mjs <version> <release-asset-dir>",
    "",
    "If --version is omitted, Cargo.toml package.version is used.",
    "If --dist is omitted, ./dist is used.",
  ].join("\n");
}

function parseArgs(argv) {
  const options = {
    version: undefined,
    distDir: undefined,
  };
  const positional = [];

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      console.log(usage());
      process.exit(0);
    }
    if (arg === "--version" || arg === "-v") {
      options.version = argv[++index];
      continue;
    }
    if (arg === "--dist" || arg === "--dir" || arg === "-d") {
      options.distDir = argv[++index];
      continue;
    }
    positional.push(arg);
  }

  if (!options.version && positional.length > 0) {
    options.version = positional.shift();
  }
  if (!options.distDir && positional.length > 0) {
    options.distDir = positional.shift();
  }
  if (positional.length > 0) {
    throw new Error(`Unexpected arguments: ${positional.join(" ")}`);
  }

  return options;
}

function readCargoVersion() {
  const cargoToml = fs.readFileSync(path.join(REPO_ROOT, "Cargo.toml"), "utf8");
  const versionMatch = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
  if (!versionMatch) {
    throw new Error("Could not find package.version in Cargo.toml");
  }
  return versionMatch[1];
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function assertPackageVersions(version) {
  const mainPackagePath = path.join(NPM_ROOT, "qtflow", "package.json");
  const mainPackage = readJson(mainPackagePath);
  if (mainPackage.version !== version) {
    throw new Error(`${mainPackagePath} version is ${mainPackage.version}, expected ${version}`);
  }

  for (const platform of PLATFORMS) {
    const dependencyVersion = mainPackage.optionalDependencies?.[platform.packageName];
    if (dependencyVersion !== version) {
      throw new Error(
        `${mainPackagePath} optional dependency ${platform.packageName} is ${dependencyVersion}, expected ${version}`,
      );
    }

    const platformPackagePath = path.join(NPM_ROOT, "platforms", platform.platform, "package.json");
    const platformPackage = readJson(platformPackagePath);
    if (platformPackage.version !== version) {
      throw new Error(`${platformPackagePath} version is ${platformPackage.version}, expected ${version}`);
    }
  }
}

function baseName(entryName) {
  return entryName.replace(/\\/g, "/").split("/").filter(Boolean).pop();
}

function extractZipMember(buffer, wantedName) {
  const end = findEndOfCentralDirectory(buffer);
  const entryCount = buffer.readUInt16LE(end + 10);
  let offset = buffer.readUInt32LE(end + 16);

  for (let index = 0; index < entryCount; index += 1) {
    if (buffer.readUInt32LE(offset) !== 0x02014b50) {
      throw new Error("Invalid ZIP central directory");
    }

    const method = buffer.readUInt16LE(offset + 10);
    const compressedSize = buffer.readUInt32LE(offset + 20);
    const uncompressedSize = buffer.readUInt32LE(offset + 24);
    const fileNameLength = buffer.readUInt16LE(offset + 28);
    const extraLength = buffer.readUInt16LE(offset + 30);
    const commentLength = buffer.readUInt16LE(offset + 32);
    const localHeaderOffset = buffer.readUInt32LE(offset + 42);
    const nameStart = offset + 46;
    const name = buffer.toString("utf8", nameStart, nameStart + fileNameLength);

    if (baseName(name) === wantedName && !name.endsWith("/")) {
      if (compressedSize === 0xffffffff || uncompressedSize === 0xffffffff) {
        throw new Error("ZIP64 archives are not supported by this helper");
      }
      return readZipLocalFile(buffer, localHeaderOffset, method, compressedSize, uncompressedSize);
    }

    offset = nameStart + fileNameLength + extraLength + commentLength;
  }

  throw new Error(`Could not find ${wantedName} in ZIP archive`);
}

function findEndOfCentralDirectory(buffer) {
  const minimum = Math.max(0, buffer.length - 0xffff - 22);
  for (let offset = buffer.length - 22; offset >= minimum; offset -= 1) {
    if (buffer.readUInt32LE(offset) === 0x06054b50) {
      return offset;
    }
  }
  throw new Error("Could not find ZIP end of central directory");
}

function readZipLocalFile(buffer, localHeaderOffset, method, compressedSize, uncompressedSize) {
  if (buffer.readUInt32LE(localHeaderOffset) !== 0x04034b50) {
    throw new Error("Invalid ZIP local file header");
  }

  const fileNameLength = buffer.readUInt16LE(localHeaderOffset + 26);
  const extraLength = buffer.readUInt16LE(localHeaderOffset + 28);
  const dataStart = localHeaderOffset + 30 + fileNameLength + extraLength;
  const compressedData = buffer.subarray(dataStart, dataStart + compressedSize);
  let data;

  if (method === 0) {
    data = Buffer.from(compressedData);
  } else if (method === 8) {
    data = zlib.inflateRawSync(compressedData);
  } else {
    throw new Error(`Unsupported ZIP compression method ${method}`);
  }

  if (data.length !== uncompressedSize) {
    throw new Error(`ZIP member size mismatch: got ${data.length}, expected ${uncompressedSize}`);
  }
  return data;
}

function extractTarGzMember(buffer, wantedName) {
  const tar = zlib.gunzipSync(buffer);
  let offset = 0;

  while (offset + 512 <= tar.length) {
    const header = tar.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) {
      break;
    }

    const name = readTarString(header, 0, 100);
    const prefix = readTarString(header, 345, 155);
    const fullName = prefix ? `${prefix}/${name}` : name;
    const sizeText = readTarString(header, 124, 12).trim();
    const size = Number.parseInt(sizeText || "0", 8);
    const typeFlag = String.fromCharCode(header[156] || 0);
    const dataStart = offset + 512;

    if ((typeFlag === "0" || typeFlag === "\0") && baseName(fullName) === wantedName) {
      return Buffer.from(tar.subarray(dataStart, dataStart + size));
    }

    offset = dataStart + Math.ceil(size / 512) * 512;
  }

  throw new Error(`Could not find ${wantedName} in tar.gz archive`);
}

function readTarString(buffer, offset, length) {
  const end = offset + length;
  let cursor = offset;
  while (cursor < end && buffer[cursor] !== 0) {
    cursor += 1;
  }
  return buffer.toString("utf8", offset, cursor);
}

function extractArchive(archivePath, archiveExt, binaryName) {
  const archive = fs.readFileSync(archivePath);
  if (archiveExt === "zip") {
    return extractZipMember(archive, binaryName);
  }
  if (archiveExt === "tar.gz") {
    return extractTarGzMember(archive, binaryName);
  }
  throw new Error(`Unsupported archive extension: ${archiveExt}`);
}

function populatePlatform(platform, version, distDir) {
  const archiveName = `qtflow-${version}-${platform.target}.${platform.archiveExt}`;
  const archivePath = path.join(distDir, archiveName);
  if (!fs.existsSync(archivePath)) {
    throw new Error(`Missing release archive: ${archivePath}`);
  }

  const binaryBytes = extractArchive(archivePath, platform.archiveExt, platform.archiveBinary);
  const packageDir = path.join(NPM_ROOT, "platforms", platform.platform);
  const binDir = path.join(packageDir, "bin");
  const outputPath = path.join(binDir, platform.packageBinary);

  fs.mkdirSync(binDir, { recursive: true });
  fs.writeFileSync(outputPath, binaryBytes);
  fs.chmodSync(outputPath, 0o755);

  console.log(`${platform.packageName}: populated ${path.relative(REPO_ROOT, outputPath)} from ${archiveName}`);
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const version = options.version ?? readCargoVersion();
  const distDir = path.resolve(REPO_ROOT, options.distDir ?? "dist");

  if (!fs.existsSync(distDir)) {
    throw new Error(`Release asset directory does not exist: ${distDir}`);
  }

  assertPackageVersions(version);
  for (const platform of PLATFORMS) {
    populatePlatform(platform, version, distDir);
  }
}

try {
  main();
} catch (error) {
  console.error(error.message);
  console.error("");
  console.error(usage());
  process.exit(1);
}
