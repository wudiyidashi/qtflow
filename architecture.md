# QtFlow Rust Architecture

## Design Principle

`qtflow` should separate command intent from command execution.

The core should build a command plan that can be printed, serialized, tested, or executed. This makes the tool reliable for humans, CI, and AI agents.

## High-Level Flow

```text
CLI args
  -> Project discovery
  -> Config loading
  -> Config/CLI/env merge
  -> Toolchain detection
  -> Command planning
  -> Environment bootstrap
  -> Command execution
  -> Diagnostic analysis
  -> Exit code
```

## Suggested Crate Layout

MVP can be one crate with modules. If the project grows, split `core` from CLI later.

```text
src/
  main.rs
  cli.rs
  config.rs
  project.rs
  plan.rs
  runner.rs
  output.rs
  error.rs
  detect/
    mod.rs
    cmake.rs
    ctest.rs
    msvc.rs
    qt.rs
  commands/
    mod.rs
    doctor.rs
    configure.rs
    build.rs
    test.rs
    check.rs
    plan.rs
  diagnostics/
    mod.rs
    rules.rs
    report.rs
```

## Core Types

```rust
pub struct ProjectContext {
    pub root: PathBuf,
    pub cmake_lists: PathBuf,
    pub presets_file: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
}

pub struct QtFlowConfig {
    pub default_profile: String,
    pub profiles: BTreeMap<String, ProfileConfig>,
    pub tools: ToolConfig,
    pub msvc: MsvcConfig,
    pub diagnostics: DiagnosticConfig,
}

pub struct ProfileConfig {
    pub preset: Option<String>,
    pub build_dir: PathBuf,
    pub generator: Option<String>,
    pub env: BTreeMap<String, String>,
    pub ctest: CTestConfig,
}

pub struct CommandPlan {
    pub label: String,
    pub cwd: PathBuf,
    pub steps: Vec<CommandStep>,
}

pub struct CommandStep {
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub bootstrap: Option<EnvironmentBootstrap>,
}

pub enum EnvironmentBootstrap {
    Msvc {
        vsdevcmd: PathBuf,
        arch: String,
        host_arch: Option<String>,
    },
}
```

## Command Planning

Planning must be pure where possible. Avoid spawning processes while building a command plan except for explicit detection steps.

Examples:

```text
configure(debug):
  cmake --preset Qt-Debug

build(debug, target=foo):
  cmake --build <build_dir> --target foo

test(debug, regex=foo):
  ctest --test-dir <build_dir> -R foo --output-on-failure

check(debug, target=foo):
  cmake --build <build_dir> --target foo
  ctest --test-dir <build_dir> -R foo --output-on-failure
```

## Windows Shell Strategy

On Windows, `VsDevCmd.bat` is a batch script and must be invoked through `cmd.exe`.

Recommended execution command:

```cmd
cmd.exe /d /s /c "call "<VsDevCmd.bat>" -arch=x64 && cmake --build ..."
```

Implementation notes:

- Keep a tested Windows quoting helper.
- Use `std::process::Command` for `cmd.exe`.
- Do not concatenate untrusted values without quoting.
- For dry-run, print a display-safe command string.

## Detection Modules

### Project Detection

- Walk upward from current working directory.
- First ancestor with `CMakeLists.txt` is project root.
- Optional config file names:
  - `.qtflow.toml`
  - `qtflow.toml`

### CMake Detection

- Use configured `tools.cmake` or `cmake`.
- `doctor` should run `cmake --version` unless `--no-probe` is set.
- Optionally parse `CMakePresets.json` to list presets.

### CTest Detection

- Use configured `tools.ctest` or `ctest`.
- `doctor` should run `ctest --version`.

### MSVC Detection

Detection order:

1. CLI path
2. `VSDEVCMD_BAT`
3. `VSINSTALLDIR`
4. `vswhere.exe`
5. known paths

Known path patterns:

```text
%ProgramFiles%\Microsoft Visual Studio\18\Community\Common7\Tools\VsDevCmd.bat
%ProgramFiles%\Microsoft Visual Studio\18\Professional\Common7\Tools\VsDevCmd.bat
%ProgramFiles%\Microsoft Visual Studio\18\Enterprise\Common7\Tools\VsDevCmd.bat
%ProgramFiles%\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat
%ProgramFiles%\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat
...
```

## Diagnostics Architecture

Diagnostics should run after command failure and inspect:

- exit code;
- command kind;
- stdout/stderr combined log;
- detected environment;
- platform.

Each rule returns zero or more findings:

```rust
pub struct DiagnosticFinding {
    pub code: String,
    pub severity: Severity,
    pub title: String,
    pub evidence: Vec<String>,
    pub explanation: String,
    pub suggested_commands: Vec<String>,
}
```

## Output Modes

Support:

- human text output by default;
- `--json` for `doctor`, `plan`, and eventually all commands;
- `--quiet` for CI;
- `--verbose` for command output and detection trace.

## Testing Strategy

Unit tests:

- config parsing and defaulting;
- config merge precedence;
- project root discovery;
- command plan generation;
- Windows command rendering;
- MSVC detection precedence;
- diagnostic matching.

Integration tests:

- `doctor --json --no-probe` in a fixture project;
- `plan check fake_test` in a fixture project;
- non-Windows command plan does not include MSVC bootstrap.

Windows smoke tests:

- `qtflow doctor --json`;
- if available, `qtflow plan check sample`.

## Distribution Architecture

### GitHub Release

Build targets:

- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `aarch64-apple-darwin`
- `x86_64-apple-darwin`

### npm Wrapper

Recommended approach:

- package name: `qtflow` or `@qtflow/cli`;
- `bin`: `qtflow`;
- wrapper script locates platform binary under package directory;
- optional platform packages can be added later.

Avoid requiring Rust toolchain on npm install.

