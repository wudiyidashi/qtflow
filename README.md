# qtflow

**English** | [中文](README.zh-CN.md)

qtflow is a Rust CLI for Qt/CMake projects that standardizes the everyday `configure`, `build`, `test`, and `check` workflow. It shells out to CMake and CTest, discovers project/config/build-directory context, and auto-initializes the MSVC developer environment on Windows when needed. It is not a replacement for CMake, CTest, Qt Creator, or Visual Studio; it is a thin, predictable workflow layer for humans and coding agents.

## Install

Before the first tagged release, build from source:

```powershell
cargo build --release
.\target\release\qtflow.exe --help
```

On Linux/macOS, the binary path is:

```sh
./target/release/qtflow --help
```

After the first `v*` GitHub release, install from a release archive:

1. Download the archive for your platform from the repository's GitHub Releases page.
2. Verify the matching `.sha256` file if desired.
3. Extract the archive.
4. Put the extracted `qtflow` or `qtflow.exe` on your `PATH`.

Release archive names:

```text
qtflow-<version>-x86_64-pc-windows-msvc.zip
qtflow-<version>-x86_64-unknown-linux-gnu.tar.gz
qtflow-<version>-x86_64-apple-darwin.tar.gz
qtflow-<version>-aarch64-apple-darwin.tar.gz
```

After the npm packages are published, install the wrapper package:

```sh
npm i -g qtflow
qtflow --help
```

The npm package installs a prebuilt binary through a platform-specific optional package, so a Rust toolchain is not required for npm installs. Until the first tagged release and manual npm publish happen, use the source build command above.

## Releasing

Release infrastructure lives in `.github/workflows/release.yml` and `npm/`. `Cargo.toml` is the source version: bump it, keep all npm package versions in sync, commit, then push a matching `v<version>` tag. The GitHub release workflow checks that the tag matches `Cargo.toml`, builds the four supported targets, creates archives and SHA256 files, and uploads them to the GitHub Release.

npm publishing is manual and credentialed. After the GitHub release exists, download the release assets, run `node npm/scripts/populate-binaries.mjs --version <version> --dist <asset-dir>`, then follow `npm/README.md` to publish the platform packages and the main `qtflow` package. Do not publish from the release workflow.

## Quick Start

Run these from a Qt/CMake project, or pass `--project <path>` from anywhere inside or outside the repo:

```powershell
qtflow doctor
qtflow init
qtflow configure --profile debug
qtflow build <target>
qtflow check <target>
qtflow test <regex> --build-target <target>
```

Preview without executing, or emit JSON for agents:

```powershell
qtflow check <target> --dry-run
qtflow plan check <target> --json
qtflow doctor --json
```

Global options:

```text
--project <path>    Project root or any path inside the project.
--config <path>     Explicit qtflow config file.
--profile <name>    Profile name. Defaults to config default_profile or debug.
--json              Emit JSON output where supported.
--quiet             Reduce output.
--verbose           Print detection trace and command details.
--dry-run           Print the command plan without executing.
--no-color          Disable ANSI colors.
```

## Commands

### doctor

Inspects the project, selected config/profile, CMake/CTest, CMake presets, build directories, MSVC bootstrap status on Windows, and configured Qt hints.

```powershell
qtflow doctor
qtflow doctor --json
qtflow doctor --no-probe
qtflow doctor --show-known-msvc
```

Key options:

- `--no-probe`: do not execute cmake, ctest, or version probes.
- `--show-known-msvc`: print known `VsDevCmd.bat` candidate paths.

### init

Creates `.qtflow.toml` and installs repo-scoped `qtflow-build-test` guidance for detected agents. It auto-detects Claude (`.claude`), Codex (`AGENTS.md` or `.codex`), and Cursor (`.cursor`). Add `--global` to also install the real reusable Codex skill under `$CODEX_HOME/skills/qtflow-build-test/` (default `~/.codex/skills/qtflow-build-test/`).

```powershell
qtflow init
qtflow init --agent codex
qtflow init --agent claude --agent cursor
qtflow init --all
qtflow init --global
qtflow init --config-only --dry-run --json
```

Key options:

- `--agent <claude|codex|cursor|all>`: select agent skill targets; repeatable.
- `--all`: install all supported agent skill files.
- `--global`: also install the global Codex skill. This can run without a project root.
- `--force`: overwrite existing qtflow-managed files.
- `--no-config`: skip `.qtflow.toml`.
- `--config-only`: create only `.qtflow.toml`.
- `--layout <vs|qtcreator|cli|presets>`: force build-dir layout selection.
- `--build-dir-debug <path>` / `--build-dir-release <path>`: override generated profile build dirs.

### configure

Runs the CMake configure step. It uses the selected profile preset when present; if no preset is available, it falls back to explicit `cmake -S <root> -B <build_dir> -G <generator>` when a generator is configured or passed.

```powershell
qtflow configure --profile debug
qtflow configure --preset Qt-Debug
qtflow configure --generator Ninja
qtflow configure --fresh
```

Key options:

- `--preset <name>`: override configured CMake preset.
- `--generator <name>`: generator override when not using a preset.
- `--fresh`: append CMake `--fresh`.
- `--no-msvc-bootstrap`: do not initialize MSVC through `VsDevCmd.bat`.
- `--vsdevcmd <path>`: explicit `VsDevCmd.bat` path.

### build

Builds one CMake target or the default/all target.

```powershell
qtflow build app
qtflow build route_dispatcher_request_build_test --profile debug
qtflow build --all
qtflow build app --build-dir out/build/debug --parallel 8
qtflow build app --config-name Debug
```

Key options:

- `<target>`: CMake target to build.
- `--build-dir <path>`: override build directory.
- `--parallel <n>`: pass `--parallel N` to `cmake --build`.
- `--all`: build the default/all target instead of a named target.
- `--config-name <name>`: pass `--config <name>` to `cmake --build` for multi-config generators.
- `--no-msvc-bootstrap`, `--vsdevcmd <path>`: MSVC bootstrap controls.

### test

Runs CTest, optionally building a target first.

```powershell
qtflow test route_dispatcher_request_build_test
qtflow test route_dispatcher --build-target route_dispatcher_request_build_test
qtflow test smoke --ctest-arg --timeout --ctest-arg 30
qtflow test route --no-output-on-failure
qtflow test smoke --config-name Release
```

Key options:

- `<regex>`: CTest regex to run.
- `--build-target <target>`: build this target before running CTest.
- `--build-dir <path>`: override build directory.
- `--config-name <name>`: pass `-C <name>` to CTest, and `--config <name>` to the optional build step.
- `--output-on-failure`: show CTest output for failing tests.
- `--no-output-on-failure`: disable CTest output-on-failure.
- `--ctest-arg <arg>`: append an extra CTest argument; repeatable.
- `--parallel <n>`: build parallelism when `--build-target` is used.
- `--no-msvc-bootstrap`, `--vsdevcmd <path>`: MSVC bootstrap controls.

### check

Builds a target, then runs the matching CTest. By default, the CTest regex is the target name.

```powershell
qtflow check route_dispatcher_request_build_test
qtflow check app --test-regex smoke
qtflow check app --parallel 8 --ctest-arg --verbose
qtflow check app --config-name Debug
```

Key options:

- `<target>`: CMake target to build before CTest.
- `--test-regex <regex>`: CTest regex; defaults to the target.
- `--build-dir <path>`: override build directory.
- `--config-name <name>`: pass `--config <name>` to the build step and `-C <name>` to CTest.
- `--parallel <n>`: build parallelism.
- `--ctest-arg <arg>`: append an extra CTest argument; repeatable.
- `--no-msvc-bootstrap`, `--vsdevcmd <path>`: MSVC bootstrap controls.

### plan

Renders the same command plan as `--dry-run`, without executing it.

```powershell
qtflow plan configure --profile debug
qtflow plan build app --profile release
qtflow plan check route_dispatcher_request_build_test
qtflow plan check route_dispatcher_request_build_test --json
```

## Configuration

qtflow reads `.qtflow.toml` or `qtflow.toml` from the project root. `--config <path>` or `QTFLOW_CONFIG` can point at an explicit file. Merge precedence is:

```text
CLI args > environment variables > .qtflow.toml > inferred defaults
```

Minimal config:

```toml
default_profile = "debug"

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"

[profiles.release]
preset = "Qt-Release"
build_dir = "out/build/release"
```

Fuller config:

```toml
default_profile = "debug"

[tools]
cmake = "cmake"
ctest = "ctest"
ninja = "ninja"

[msvc]
enabled = true
arch = "x64"
host_arch = "x64"
vsdevcmd = ""

[qt]
root = "C:/Qt/6.8.0/msvc2022_64"
bin_dir = "C:/Qt/6.8.0/msvc2022_64/bin"

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
generator = "Ninja"
configure_args = []
build_args = []
ctest_args = ["--output-on-failure"]

[profiles.debug.env]
QT_LOGGING_RULES = "qt.qml=false"

[profiles.release]
preset = "Qt-Release"
build_dir = "out/build/release"
generator = "Ninja"
configure_args = []
build_args = []
ctest_args = ["--output-on-failure"]

[tests.route_dispatcher]
target = "route_dispatcher_request_build_test"
regex = "route_dispatcher_request_build_test"
profile = "debug"

[diagnostics]
enabled = true
max_log_bytes = 200000
```

Configuration fields:

- `default_profile`: profile used when `--profile` is omitted.
- `[tools].cmake`, `[tools].ctest`, `[tools].ninja`: executable names or absolute paths.
- `[msvc].enabled`, `arch`, `host_arch`, `vsdevcmd`: Windows MSVC bootstrap settings.
- `[qt].root`, `[qt].bin_dir`: Qt hints reported by `doctor`.
- `[profiles.<name>].preset`: CMake preset name.
- `[profiles.<name>].build_dir`: build directory, relative to project root unless absolute.
- `[profiles.<name>].generator`: generator for explicit `cmake -S/-B/-G` configure when no preset is used.
- `[profiles.<name>].config_name`: optional build/test configuration for multi-config generators. CLI `--config-name` overrides the profile value.
- `[profiles.<name>].configure_args`, `build_args`, `ctest_args`: extra arguments appended to generated commands.
- `[profiles.<name>.env]`: environment variables added to command steps.
- `[diagnostics]`: enable/disable diagnostics and bound captured log size.

Environment variables:

```text
QTFLOW_CONFIG          Explicit config path.
QTFLOW_PROFILE         Default profile override.
QTFLOW_CMAKE           CMake executable override.
QTFLOW_CTEST           CTest executable override.
QTFLOW_VSDEVCMD_BAT    VsDevCmd.bat path override.
VSDEVCMD_BAT           Compatibility VsDevCmd.bat path override.
```

## Build-Directory Discovery and Visual Studio

`qtflow init` and `qtflow doctor` scan for real build directories by finding `CMakeCache.txt` files that belong to the project. qtflow reads cache fields such as `CMAKE_HOME_DIRECTORY`, `CMAKE_BUILD_TYPE`, and `CMAKE_GENERATOR`, classifies debug and release directories, recognizes multi-config generators, and filters common dependency/noise directories.

Visual Studio-generated directories under patterns such as `out/build/...` are detected when the cache carries Visual Studio provenance, including generated-by comments or `CMAKE_GENERATOR_INSTANCE` paths. When more than one debug or release candidate exists, qtflow chooses deterministically, reports alternates, and tells you how to override.

For generated config:

```powershell
qtflow init --layout vs
qtflow init --layout qtcreator
qtflow init --layout cli
qtflow init --layout presets
qtflow init --build-dir-debug out/build/x64-Debug --build-dir-release out/build/x64-Release
```

Layout meanings:

- `vs`: prefer discovered Visual Studio dirs, otherwise use `out/build/x64-Debug` and `out/build/x64-Release`.
- `qtcreator`: prefer preset `binaryDir` values when usable, otherwise use `build/Debug` and `build/Release`.
- `cli`: use `build` and `build-release` with `Ninja`.
- `presets`: require usable CMake configure presets with `binaryDir`.

## Multi-Config Generators

Visual Studio, Xcode, and Ninja Multi-Config choose Debug/Release at build and test time. Use `--config-name <name>` for one-off commands:

```powershell
qtflow build app --config-name Debug
qtflow test smoke --config-name Release
qtflow check app --config-name Debug
```

When set on a profile, `config_name` is used by `build`, `test`, and `check`; the CLI flag overrides the profile value. `qtflow init` writes `config_name = "Debug"` and `config_name = "Release"` automatically when it discovers a multi-config build directory, because one build directory serves both profiles and CMake/CTest need the configuration at build/test time.

```toml
[profiles.debug]
build_dir = "out/build/vs"
generator = "Visual Studio 17 2022"
config_name = "Debug"

[profiles.release]
build_dir = "out/build/vs"
generator = "Visual Studio 17 2022"
config_name = "Release"
```

## Windows / MSVC

On Windows, qtflow initializes the MSVC environment before planned configure/build/test steps when `[msvc].enabled = true` and the command does not pass `--no-msvc-bootstrap`. It resolves `VsDevCmd.bat`, then executes the step through `cmd.exe` with `call "<VsDevCmd.bat>" -arch=<arch>`.

Detection precedence:

1. `--vsdevcmd <path>`
2. `QTFLOW_VSDEVCMD_BAT`
3. `VSDEVCMD_BAT`
4. `[msvc].vsdevcmd`
5. `VSINSTALLDIR\Common7\Tools\VsDevCmd.bat`
6. `vswhere.exe`
7. Known Visual Studio install paths

Useful commands:

```powershell
qtflow doctor --show-known-msvc
qtflow build app --no-msvc-bootstrap
qtflow check app --vsdevcmd "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"
$env:QTFLOW_VSDEVCMD_BAT = "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"
```

Use `--no-msvc-bootstrap` when you are already inside a Visual Studio Developer Command Prompt or when the project intentionally uses another environment.

## Agent Integration

`qtflow init` installs `qtflow-build-test` guidance so agents use qtflow instead of reconstructing raw CMake, CTest, or Visual Studio Developer Prompt commands.

Repo-scoped targets:

- Claude: `.claude/skills/qtflow-build-test/SKILL.md`
- Codex: managed `qtflow-build-test` section in `AGENTS.md`
- Cursor: `.cursor/rules/qtflow-build-test.mdc`

Codex only auto-loads skills from the global skills directory. Use `qtflow init --global` to install the canonical global skill at `$CODEX_HOME/skills/qtflow-build-test/` or `~/.codex/skills/qtflow-build-test/` when `CODEX_HOME` is unset. It writes `SKILL.md` and `agents/openai.yaml`; restart Codex after installation.

Examples:

```powershell
qtflow init
qtflow init --agent codex
qtflow init --agent claude --agent cursor
qtflow init --global
qtflow init --all --dry-run --json
```

All commands that support `--json` emit stable, agent-consumable output. Use JSON for planning and diagnostics when an agent needs structured data.

## Diagnostics

Failed commands preserve the original tool output and add structured diagnostics when a known setup issue is detected. Text output includes evidence, why it likely happened, and suggested fixes. With `--json`, failures emit an object with `exitCode` and `diagnostics`.

Rule codes:

- `msvc.missing_standard_headers`: MSVC standard headers such as `string`, `vector`, or `stddef.h` are unavailable.
- `msvc.vsdevcmd_not_found`: `VsDevCmd.bat` could not be located.
- `cmake.build_dir_missing`: selected build directory is missing or has no usable cache.
- `cmake.preset_missing`: configured CMake preset is missing.
- `tool.cmake_not_found`: CMake executable could not be started.
- `tool.ctest_not_found`: CTest executable could not be started.
- `qt.runtime_dll_missing`: Qt runtime DLLs are missing at test time.
- `ctest.no_tests_matched`: CTest ran but matched no tests.

## Exit Codes

| Code | Meaning |
|---:|---|
| 0 | Success. |
| 1 | Command executed but failed. |
| 2 | Configuration or argument error. |
| 3 | Required tool not found. |
| 4 | Project root/config not found. |
| 5 | Environment bootstrap failed. |
| 6 | Diagnostic found a known fatal setup issue. |

## JSON Output

Plan example:

```powershell
qtflow plan check route_dispatcher_request_build_test --json
```

Example shape:

```json
{
  "projectRoot": "D:/repo",
  "profile": "debug",
  "steps": [
    {
      "label": "build",
      "cwd": "D:/repo",
      "program": "cmake",
      "args": [
        "--build",
        "D:/repo/out/build/debug",
        "--target",
        "route_dispatcher_request_build_test"
      ]
    },
    {
      "label": "test",
      "cwd": "D:/repo",
      "program": "ctest",
      "args": [
        "--test-dir",
        "D:/repo/out/build/debug",
        "-R",
        "route_dispatcher_request_build_test",
        "--output-on-failure"
      ]
    }
  ]
}
```

On Windows, bootstrapped steps also include a `bootstrap` object:

```json
{
  "kind": "msvc",
  "vsdevcmd": "C:/Program Files/Microsoft Visual Studio/2022/BuildTools/Common7/Tools/VsDevCmd.bat",
  "arch": "x64"
}
```

Diagnostics JSON example:

```json
{
  "exitCode": 1,
  "diagnostics": [
    {
      "code": "msvc.missing_standard_headers",
      "severity": "error",
      "title": "MSVC standard headers are unavailable",
      "evidence": [
        "fatal error C1083: cannot open include file: 'string'"
      ],
      "explanation": "The build likely ran without Visual Studio developer environment variables.",
      "suggestedCommands": [
        "qtflow doctor",
        "set QTFLOW_VSDEVCMD_BAT=<path-to-VsDevCmd.bat>",
        "qtflow build <target>"
      ]
    }
  ]
}
```
