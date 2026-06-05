# QtFlow Rust CLI PRD

## Product Summary

`qtflow` is a standalone Rust command-line tool for Qt/CMake projects. It standardizes local build and test workflows, especially on Windows/MSVC where Qt/CMake commands often fail because the Visual Studio developer environment is not initialized.

The product is not a compiler, build system, or CMake replacement. It is a workflow orchestrator:

```text
discover project -> read config -> detect toolchain -> plan commands -> bootstrap environment -> execute -> diagnose failures
```

## Target Users

- C++/Qt developers using CMake.
- Teams with repeatable debug/release build directories and CTest targets.
- AI coding agents that need deterministic build/test commands.
- CI maintainers who want one local command surface matching CI steps.

## Problem Statement

Qt/CMake projects often have local build workflows that are simple in principle but fragile in practice:

- A normal Windows shell can find `cl.exe` but not the full MSVC include/library environment.
- Developers forget which CMake preset maps to debug or release.
- Build directories differ between projects.
- CTest names may differ from CMake targets.
- Qt runtime DLL/PATH issues are hard to diagnose from raw logs.
- AI agents repeatedly rediscover the same build commands and environment fixes.

## Goals

- Provide one stable CLI for configure/build/test/check workflows.
- Make Windows/MSVC environment setup automatic and explainable.
- Keep all commands inspectable with `--dry-run`.
- Support project-specific configuration without hardcoding paths in the CLI.
- Emit diagnostics that point to the next practical fix.
- Be suitable for GitHub releases and npm distribution through prebuilt binaries.
- Provide an agent skill template so Codex/Claude/Cursor-style agents use the CLI instead of raw CMake.

## Non-Goals

- Replace CMake, Ninja, MSBuild, or CTest.
- Generate Qt project files.
- Manage Qt installation.
- Implement a full CI system.
- Parse every compiler error semantically in MVP.
- Support qmake as a first-class workflow in MVP.

## MVP Scope

### Commands

- `qtflow doctor`
- `qtflow configure`
- `qtflow build`
- `qtflow test`
- `qtflow check`
- `qtflow plan`

### Platforms

- MVP primary: Windows with MSVC and Qt/CMake.
- MVP secondary: Linux/macOS pass-through mode for CMake/CTest without MSVC bootstrapping.

### Configuration

- Read `.qtflow.toml` from project root.
- If no config exists, infer conservative defaults:
  - project root: nearest ancestor containing `CMakeLists.txt`;
  - debug build dir: `out/build/debug`;
  - release build dir: `out/build/release`;
  - debug preset: `Qt-Debug` if present, otherwise no default preset;
  - release preset: `Qt-Release` if present, otherwise no default preset.

### MSVC Detection

Detection order:

1. `--vsdevcmd <path>`
2. `VSDEVCMD_BAT`
3. `VSINSTALLDIR/Common7/Tools/VsDevCmd.bat`
4. `vswhere.exe`
5. known Visual Studio install paths

### Command Execution

- Build commands should call `cmake --build <build-dir>`.
- Test commands should call `ctest --test-dir <build-dir>`.
- On Windows/MSVC, commands should run through:

```cmd
cmd.exe /d /c call "<VsDevCmd.bat>" -arch=x64 && <command>
```

- `--dry-run` prints the exact command plan without executing.

## User Stories

### Doctor

As a developer, I can run `qtflow doctor` and see whether the project root, CMake, CTest, build directories, Qt path hints, and MSVC developer command are detected.

### Focused Check

As an AI agent, I can run:

```powershell
qtflow check route_dispatcher_request_build_test
```

and the tool builds the target then runs the matching CTest regex.

### CTest Name Differs From Target

As a developer, I can run:

```powershell
qtflow test route_dispatcher --build-target route_dispatcher_request_build_test
```

and the tool builds the target before running `ctest -R route_dispatcher`.

### Inspect Before Run

As a maintainer, I can run:

```powershell
qtflow check my_test --dry-run
```

and inspect the exact CMake/CTest commands and environment bootstrap.

### Fix Missing MSVC Environment

As a Windows developer, when raw CMake would fail with missing standard headers, `qtflow` automatically runs through `VsDevCmd.bat`.

## Acceptance Criteria

- `qtflow doctor` works outside a Visual Studio Developer Prompt.
- `qtflow check <target>` builds a CMake target and runs matching CTest.
- `qtflow test <regex> --build-target <target>` builds first, then runs CTest.
- `qtflow configure --profile debug` runs the configured debug preset.
- `--dry-run` prints exact commands and exits without execution.
- `.qtflow.toml` can override profiles, presets, build dirs, env, CTest options, and MSVC behavior.
- Windows MSVC detection supports `VSDEVCMD_BAT`, `VSINSTALLDIR`, `vswhere`, and known paths.
- Non-Windows platforms do not require MSVC detection.
- Common failures produce actionable diagnostic messages.
- Exit codes are stable and documented.
- npm package can invoke a prebuilt binary or clearly fail with install instructions.

## Definition Of Done

- Rust CLI implemented with unit tests for config merge, project detection, command planning, MSVC detection path selection, and diagnostics.
- Integration tests cover command planning without requiring Qt.
- At least one Windows smoke test validates `doctor --json`.
- README includes installation, examples, config schema, and agent usage.
- GitHub release workflow builds binaries for Windows, Linux, and macOS.
- npm wrapper publishes a package with `bin` command `qtflow`.

## Risks

- Visual Studio installations vary by version and edition.
- Some Qt projects require environment variables beyond standard MSVC setup.
- Some projects use CMake presets with custom names.
- Test target and CTest names may not map 1:1.
- Shell quoting on Windows is easy to get wrong.

## Open Decisions

- Whether `qtflow configure` should infer CMake presets from `CMakePresets.json` when `.qtflow.toml` is absent.
- Whether MVP should support Qt DLL deployment checks.
- Whether npm package should download binaries during postinstall or bundle platform-specific optional dependencies.
- Whether qmake support should be a separate product or later command group.

