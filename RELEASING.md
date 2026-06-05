# Releasing qtflow

Use `scripts/release.mjs` from the repository root. It uses only Node built-in modules and never stores npm credentials in the repo.

## Version sync map

All release versions must match:

- `Cargo.toml` package `version`
- `Cargo.lock` `qtflow` package entry
- `npm/qtflow/package.json` `version`
- `npm/qtflow/package.json` `optionalDependencies` for all platform packages
- `npm/platforms/win32-x64/package.json` `version`
- `npm/platforms/linux-x64/package.json` `version`
- `npm/platforms/darwin-x64/package.json` `version`
- `npm/platforms/darwin-arm64/package.json` `version`

The GitHub Release workflow also rejects tags where `v<X>` does not match `Cargo.toml`.

## Flow

1. Bump, commit, tag, and push:

   ```sh
   node scripts/release.mjs bump <X> --commit --push
   ```

   Or review first:

   ```sh
   node scripts/release.mjs bump <X>
   node scripts/release.mjs check
   git add -A
   git commit -m "release: v<X>"
   git tag v<X>
   git push --follow-tags
   ```

2. Wait for the GitHub Actions `Release` workflow to finish. It builds the four target archives and uploads them to the GitHub Release for `v<X>`.

3. Publish npm packages:

   ```sh
   node scripts/release.mjs publish <X>
   ```

   Run this with `NPM_TOKEN` or `NODE_AUTH_TOKEN` set to an npm **Automation** token. Automation tokens bypass 2FA for publishing. Classic `Publish` tokens and granular tokens without 2FA bypass can return `403`; in that case either use an Automation token or pass a one-time password:

   ```sh
   node scripts/release.mjs publish <X> --otp <code>
   ```

   The script reads `NODE_AUTH_TOKEN` or `NPM_TOKEN`, writes a temporary npm userconfig outside the repo, runs `npm publish`, and deletes the temporary file. If no token is set, it uses your existing npm login.

4. Verify:

   ```sh
   npm view qtflow version
   npm i -g qtflow
   ```

## Dry run

To test the npm packaging path before a real publish:

```sh
node scripts/release.mjs publish <X> --dry-run
```

`publish` first runs the version check, downloads `v<X>` assets with `gh release download`, populates `npm/platforms/*/bin/`, then publishes the four platform packages followed by `npm/qtflow`. With `--dry-run`, npm receives `npm publish --dry-run` for each package.
