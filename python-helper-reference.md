# Python Helper Reference And Rust Migration Notes

## Current Helper Summary

The current project-local helper is a Python script with these commands:

```text
python scripts/qt_workflow.py doctor
python scripts/qt_workflow.py configure --config debug
python scripts/qt_workflow.py build <target>
python scripts/qt_workflow.py test <regex> --build-target <target>
python scripts/qt_workflow.py check <target>
```

Its important behavior:

- finds repo root by walking upward to `CMakeLists.txt`;
- defaults debug build dir to `out/build/debug`;
- defaults release build dir to `out/build/release`;
- defaults debug preset to `Qt-Debug`;
- defaults release preset to `Qt-Release`;
- detects `VsDevCmd.bat` from:
  - `VSDEVCMD_BAT`;
  - `VSINSTALLDIR`;
  - `vswhere.exe`;
  - known Visual Studio paths;
- runs Windows commands through:

```cmd
cmd.exe /d /c call "<VsDevCmd.bat>" -arch=x64 && <command>
```

- builds targets through:

```text
cmake --build <build_dir> --target <target>
```

- runs tests through:

```text
ctest --test-dir <build_dir> -R <regex> --output-on-failure
```

## What To Preserve In Rust

Preserve these decisions:

- command surface: `doctor`, `configure`, `build`, `test`, `check`;
- `check` means build target then run matching CTest;
- Windows MSVC bootstrap is automatic by default;
- `--no-msvc-bootstrap` exists for initialized developer prompts;
- `--dry-run` prints exact commands;
- focused test workflow is the default quality gate for agents.

## What To Improve In Rust

Add:

- `.qtflow.toml` config;
- JSON output;
- explicit command plan model;
- stable exit codes;
- diagnostics;
- npm/GitHub release packaging;
- robust Windows quoting tests;
- fixture-based integration tests.

## Migration Map

| Python concept | Rust target |
|---|---|
| `find_repo_root` | `project::discover_root` |
| `DEFAULT_BUILD_DIRS` | config inferred defaults |
| `DEFAULT_PRESETS` | config inferred defaults plus CMakePresets validation |
| `find_vsdevcmd` | `detect::msvc::detect_vsdevcmd` |
| `run_command` | `runner::execute_plan` |
| `build_args` | `commands::build::plan` |
| `test_args` | `commands::test::plan` |
| `cmd_check` | `commands::check::plan` with two steps |

## Current Limitations To Avoid

- Hardcoded preset names should move into config.
- Hardcoded build dirs should move into config.
- Python script has no structured command plan output.
- Python script has no diagnostic rule engine.
- Python script has no install/distribution model.
- Python script does not validate `CMakePresets.json`.
- Python script does not expose JSON output for agents.

