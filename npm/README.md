# qtflow npm publishing

This directory contains npm packaging infrastructure only. Nothing here publishes automatically, and no npm token is stored in this repository.

## Packages

- `npm/qtflow`: main `qtflow` package. It installs `bin/qtflow.js`, a dependency-free Node shim.
- `npm/platforms/win32-x64`: `@xehxx/qtflow-cli-win32-x64`, populated with `bin/qtflow.exe`.
- `npm/platforms/linux-x64`: `@xehxx/qtflow-cli-linux-x64`, populated with `bin/qtflow`.
- `npm/platforms/darwin-x64`: `@xehxx/qtflow-cli-darwin-x64`, populated with `bin/qtflow`.
- `npm/platforms/darwin-arm64`: `@xehxx/qtflow-cli-darwin-arm64`, populated with `bin/qtflow`.

The main package uses optional dependencies so npm installs only the platform package that matches the user's `os` and `cpu`.

## Target mapping

| npm os-arch | Rust target triple | GitHub release archive |
|---|---|---|
| `win32-x64` | `x86_64-pc-windows-msvc` | `qtflow-<version>-x86_64-pc-windows-msvc.zip` |
| `linux-x64` | `x86_64-unknown-linux-gnu` | `qtflow-<version>-x86_64-unknown-linux-gnu.tar.gz` |
| `darwin-x64` | `x86_64-apple-darwin` | `qtflow-<version>-x86_64-apple-darwin.tar.gz` |
| `darwin-arm64` | `aarch64-apple-darwin` | `qtflow-<version>-aarch64-apple-darwin.tar.gz` |

## Manual publish flow

1. Bump `Cargo.toml` version. Cargo is the source version.
2. Bump `npm/qtflow/package.json`, all `npm/platforms/*/package.json` files, and the main package `optionalDependencies` to the same version.
3. Commit the version bump, initialize/configure the GitHub repo if needed, and push a matching `v<version>` tag.
4. Wait for `.github/workflows/release.yml` to create GitHub release assets.
5. Download the release archives into a local directory, for example `dist/`.
6. Populate the platform packages:

   ```sh
   node npm/scripts/populate-binaries.mjs --version <version> --dist dist
   ```

7. Inspect each package:

   ```sh
   npm pack ./npm/platforms/win32-x64 --dry-run
   npm pack ./npm/platforms/linux-x64 --dry-run
   npm pack ./npm/platforms/darwin-x64 --dry-run
   npm pack ./npm/platforms/darwin-arm64 --dry-run
   npm pack ./npm/qtflow --dry-run
   ```

8. Publish the platform packages first, then publish the main package. Scoped public packages require `npm publish --access public`:

   ```sh
   npm publish ./npm/platforms/win32-x64 --access public
   npm publish ./npm/platforms/linux-x64 --access public
   npm publish ./npm/platforms/darwin-x64 --access public
   npm publish ./npm/platforms/darwin-arm64 --access public
   npm publish ./npm/qtflow --access public
   ```

Publishing is a manual, credentialed maintainer action. Do not run these commands from this infrastructure task.

## Version sync

`Cargo.toml` is the source version. The release workflow rejects tags that do not match it. npm package versions must be bumped manually to the same version before publishing; `populate-binaries.mjs` also checks the npm metadata before copying binaries.
