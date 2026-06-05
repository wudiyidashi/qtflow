# QtFlow Rust — Design & Implementation Plan

Status: design proposal for the `qtflow` MVP.
Scope: satisfies `AI_HANDOFF.md` required outputs under its stated constraints.

## 0. Guiding Architecture Decision

**Planning is pure; execution is a thin shell.**

Every subcommand resolves context, then produces a `CommandPlan` *value* (no process spawned). Execution is a separate step that consumes a plan. `--dry-run` and `qtflow plan <cmd>` simply stop after planning and render.

This single decision discharges most constraints:

| Constraint | How this satisfies it |
|---|---|
| Keep command planning testable without Qt or Visual Studio | Planning is data-in/data-out; tests assert on plan structs, no toolchain needed |
| Windows/MSVC reliability first-class | Bootstrap is an explicit, serialized, unit-tested field of the plan, not an implicit side effect |
| Output useful for humans and agents | One plan, two renderers (text + JSON). Agents read JSON; humans read text |
| Do not replace CMake/CTest | Plans only ever wrap `cmake`/`ctest` invocations; qtflow never compiles anything |
| No qmake in MVP | No qmake types exist in the plan model; adding it later is an additive enum variant |

Detection (which *does* touch the system) is isolated behind a `Probe` trait so tests inject fakes.

---

## 1. Crate / Module Structure

**Decision: single binary crate for MVP, internally split into a `core` library boundary**, so a later `cargo` workspace split (`qtflow-core` lib + `qtflow` bin) is a move, not a rewrite.

Why not a workspace now: premature; one crate keeps build/test/release simple. Why keep the boundary: the pure planning/config/diagnostics code must never depend on `clap` or `std::process`, so a future extraction (e.g. to embed in an IDE plugin) stays cheap.

```text
qtflow/
  Cargo.toml
  src/
    main.rs            # bin entry: parse -> dispatch -> map error to exit code
    lib.rs             # re-exports core; the "library boundary"
    cli.rs             # clap types (GlobalArgs, Cli, Command enum) ONLY
    app.rs             # dispatch: clap types -> core calls -> output
    error.rs           # QtflowError + ExitCode mapping

    core/
      mod.rs
      project.rs       # discover_root, locate config/presets   (pure + 1 fs walk)
      config/
        mod.rs
        raw.rs         # serde structs, all Option<_> (the TOML shape)
        model.rs       # ResolvedConfig, Profile (no Options, fully defaulted)
        merge.rs       # raw + env + cli + defaults -> ResolvedConfig
      plan.rs          # CommandPlan, CommandStep, EnvironmentBootstrap (+ serde)
      planners/        # PURE: ctx -> CommandPlan
        mod.rs
        configure.rs
        build.rs
        test.rs
        check.rs
      detect/
        mod.rs         # Probe trait (injectable), Detected* result types
        cmake.rs
        ctest.rs
        msvc.rs        # VsDevCmd resolution precedence
        qt.rs
      runner/
        mod.rs         # execute_plan(plan) -> RunOutcome  (the only std::process user besides probes)
        shell.rs       # Windows cmd.exe + VsDevCmd wrapping, quoting helper
      diagnostics/
        mod.rs         # Engine, DiagnosticContext, Finding
        rules.rs       # static RULES table
        report.rs      # render findings (text/json)
    render/
      mod.rs           # text vs json renderers for doctor/plan/run/diagnostics
  tests/
    fixtures/          # sample projects (CMakeLists.txt, CMakePresets.json, .qtflow.toml)
    plan_*.rs          # integration: plan rendering
    doctor_*.rs        # integration: doctor --no-probe
    windows_smoke.rs   # #[cfg(windows)] gated
```

Dependency rule (enforced by review + module layering): `cli.rs`/`app.rs`/`render` may use `core`; `core::planners`, `core::config`, `core::plan`, `core::diagnostics` must **not** import `clap`, `std::process`, or `std::env` directly (env arrives as an injected map). `runner` and `detect` are the only modules allowed to touch the OS.

### Dependencies (Cargo)

- `clap` (v4, `derive`) — parsing.
- `serde` + `serde_json` — JSON output, plan serialization.
- `toml` — config parsing.
- `anyhow` (bin/app edges) + `thiserror` (typed `QtflowError` in core).
- `which` — locate `cmake`/`ctest`/`vswhere` on PATH (cross-platform).
- Dev: `insta` (snapshot tests for plans/output), `assert_cmd` + `predicates` (CLI integration), `tempfile` (fixture trees).

No regex crate required for diagnostics MVP (substring matching is sufficient and faster to reason about); revisit only if a rule needs real patterns.

---

## 2. Command Parsing Approach

**Decision: `clap` v4 derive.** Global options live in a `#[command(flatten)] GlobalArgs`; subcommands are a `Command` enum. `plan` is modeled as a subcommand that *wraps another command*.

```rust
#[derive(Parser)]
#[command(name = "qtflow", version)]
struct Cli {
    #[command(flatten)]
    global: GlobalArgs,
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Clone)]
struct GlobalArgs {
    #[arg(long, global = true)] project: Option<PathBuf>,
    #[arg(long, global = true)] config: Option<PathBuf>,
    #[arg(long, global = true)] profile: Option<String>,
    #[arg(long, global = true)] json: bool,
    #[arg(long, global = true)] quiet: bool,
    #[arg(long, global = true)] verbose: bool,
    #[arg(long = "dry-run", global = true)] dry_run: bool,
    #[arg(long = "no-color", global = true)] no_color: bool,
}

#[derive(Subcommand)]
enum Command {
    Doctor(DoctorArgs),
    Configure(ConfigureArgs),
    Build(BuildArgs),
    Test(TestArgs),
    Check(CheckArgs),
    Plan { #[command(subcommand)] inner: PlanTarget }, // configure/build/test/check
}
```

Key points:
- **`plan <cmd>` reuses the same arg structs** as the real commands by sharing `ConfigureArgs`/`BuildArgs`/… inside `PlanTarget`. The contract says `plan X` ≡ `X --dry-run` but without execution setup; we implement it by routing both to the same planner and forcing "no execution" for the `plan` path. No duplicated flag definitions.
- `--dry-run` is global so `qtflow build app --dry-run` also works (contract requires both forms).
- Mutually-exclusive flags (`--output-on-failure` / `--no-output-on-failure`) use clap `overrides_with`; we store the resolved bool.
- Parsing errors map to **exit code 2** (clap's `Error` → our `ConfigOrArg` error).
- `app.rs` translates clap structs into a single `Invocation` value passed to core, so core never sees clap types.

---

## 3. Config Model

**Decision: three-layer config.** Raw (TOML shape, every field `Option`) → defaults → merge with env + CLI → `ResolvedConfig` (no `Option`s on required fields). Merge precedence per `config-contract.md`: **CLI > env > file > inferred defaults**.

```rust
// raw.rs — exactly the TOML shape, tolerant
#[derive(Deserialize, Default)]
struct RawConfig {
    default_profile: Option<String>,
    tools: Option<RawTools>,
    msvc: Option<RawMsvc>,
    qt: Option<RawQt>,
    #[serde(default)] profiles: BTreeMap<String, RawProfile>,
    #[serde(default)] tests: BTreeMap<String, RawTestPreset>,
    diagnostics: Option<RawDiagnostics>,
}

// model.rs — resolved, total
struct ResolvedConfig {
    default_profile: String,
    tools: Tools,            // cmake/ctest/ninja, defaulted to bare names
    msvc: MsvcConfig,        // enabled, arch="x64", host_arch, vsdevcmd: Option<PathBuf>
    qt: QtConfig,
    profiles: BTreeMap<String, Profile>, // always has debug+release after defaulting
    tests: BTreeMap<String, TestPreset>,
    diagnostics: DiagnosticsConfig,
    source: ConfigSource,    // file path or "<inferred>" — surfaced by doctor
}

struct Profile {
    preset: Option<String>,  // may legitimately be None -> configure errors clearly
    build_dir: PathBuf,      // resolved absolute against project root
    generator: Option<String>,
    configure_args: Vec<String>,
    build_args: Vec<String>,
    ctest_args: Vec<String>, // defaults to ["--output-on-failure"]
    env: BTreeMap<String, String>,
}
```

Resolution pipeline (pure function, env passed in as a map):
1. `discover` config file (`--config` → `QTFLOW_CONFIG` → `.qtflow.toml` → `qtflow.toml` at project root). Absent file is not an error; inferred defaults apply.
2. Parse to `RawConfig`; an *invalid* TOML is exit code 2 with line context.
3. Apply inferred defaults (debug→`out/build/debug`/`Qt-Debug`, release→`out/build/release`/`Qt-Release`).
4. Overlay env vars (`QTFLOW_PROFILE`, `QTFLOW_CMAKE`, `QTFLOW_CTEST`, `QTFLOW_VSDEVCMD_BAT`, `VSDEVCMD_BAT`).
5. Overlay CLI (`--profile`, `--config`, tool overrides if added, `--vsdevcmd`, `--no-msvc-bootstrap`).
6. Resolve `build_dir` to absolute paths; select the active `Profile` (CLI `--profile` else `default_profile` else `"debug"`).

`build_dir` and presets are **never hardcoded in Rust** — they live only in defaults/config (fixing a Python-helper limitation). Unknown TOML keys are ignored (forward-compatible) but `--verbose` lists them.

---

## 4. Command Plan Model

**Decision: the plan is the product.** It is fully serializable, ordered, and self-describing. Planners are pure functions `fn plan(ctx: &PlanContext) -> Result<CommandPlan, QtflowError>`.

```rust
#[derive(Serialize, Clone)]
struct CommandPlan {
    project_root: PathBuf,
    profile: String,
    steps: Vec<CommandStep>,
}

#[derive(Serialize, Clone)]
struct CommandStep {
    label: String,                 // "configure" | "build" | "test"
    cwd: PathBuf,
    program: String,               // "cmake" | "ctest" (resolved tool)
    args: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bootstrap: Option<EnvironmentBootstrap>,
}

#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum EnvironmentBootstrap {
    Msvc { vsdevcmd: PathBuf, arch: String, #[serde(skip_serializing_if="Option::is_none")] host_arch: Option<String> },
    // future: Other variants are additive
}
```

Planner outputs (match `cli-contract.md` exactly):

```text
configure(debug)         -> cmake --preset Qt-Debug  [+ configure_args]
build(debug, foo)        -> cmake --build <dir> --target foo [--parallel n] [+ build_args]
test(debug, regex=foo)   -> [optional build step] ; ctest --test-dir <dir> -R foo --output-on-failure [+ ctest_args]
check(debug, foo)        -> cmake --build <dir> --target foo ; ctest --test-dir <dir> -R <regex|foo> --output-on-failure
```

JSON output matches the `cli-contract.md` example byte-for-field. The `bootstrap` field is attached per-step by the planner *only when* `cfg!(windows) && msvc.enabled && !--no-msvc-bootstrap`. On non-Windows it is always `None` (a unit-tested invariant). This is why MSVC reliability is "first-class": it is visible data, asserted in tests, not a runtime surprise.

---

## 5. Windows / MSVC Bootstrap Strategy

**Detection precedence** (`detect::msvc::resolve_vsdevcmd`), first hit wins, each candidate validated by file existence:

1. `--vsdevcmd <path>` (CLI)
2. `QTFLOW_VSDEVCMD_BAT`
3. `VSDEVCMD_BAT` (compat)
4. `[msvc].vsdevcmd` from config
5. `VSINSTALLDIR/Common7/Tools/VsDevCmd.bat`
6. `vswhere.exe` (`-latest -products * -property installationPath`, then append `Common7/Tools/VsDevCmd.bat`)
7. Known install paths (VS 2022 + "18", editions Community/Professional/Enterprise/BuildTools, under both `%ProgramFiles%` and `%ProgramFiles(x86)%`)

Returns `Resolved { path, source }` or `NotFound { searched: Vec<Source> }`. `doctor --show-known-msvc` prints candidate list; `doctor` reports which source matched.

**Execution wrapping** (`runner::shell`): `VsDevCmd.bat` is a batch script, so on Windows a bootstrapped step runs as:

```cmd
cmd.exe /d /s /c "call "<VsDevCmd.bat>" -arch=x64 && <program> <args...>"
```

Implementation rules:
- A single **tested quoting helper** `quote_for_cmd(parts) -> String` owns all escaping. Paths with spaces are the norm (`C:\Program Files\...`), so this is unit-tested against known-tricky inputs (spaces, embedded quotes, `&`, `^`).
- We build the `cmd.exe` argument as one string (the `/c "..."` form) using `std::process::Command::raw_arg` semantics carefully, or pass the composed command as a single arg — covered by Windows smoke tests.
- For `--dry-run`/`plan`, we render a **display-safe** string of the same command (never executed), so what you inspect is what would run.
- Non-Windows: no wrapping; `program`/`args` run directly. `--no-msvc-bootstrap` forces the direct path on Windows too (for use inside an already-initialized Developer Prompt).
- Failure to locate `VsDevCmd.bat` when bootstrap is required → exit code **5** (`environment bootstrap failed`) with the `msvc.vsdevcmd_not_found` diagnostic.

Optional later optimization (not MVP): run `VsDevCmd.bat && set` once, cache the env delta, and inject vars directly to avoid a `cmd.exe` per step. Deferred because per-step wrapping is simpler and correct.

---

## 6. Diagnostics Rule Engine

**Decision: data-driven rule table over a `Rule` trait.** Each rule is a static descriptor with substring patterns; the engine scans the combined (stdout+stderr, truncated to `max_log_bytes`) log after a non-zero exit. Data-driven keeps rules declarative, trivially testable, and easy to extend.

```rust
struct DiagnosticContext<'a> {
    exit_code: i32,
    command_kind: CommandKind,     // Configure | Build | Test
    combined_log: &'a str,
    platform: Platform,
    bootstrap_used: bool,
}

struct Finding {
    code: String, severity: Severity,
    title: String, evidence: Vec<String>,
    explanation: String, suggested_commands: Vec<String>,
}

struct Rule {
    code: &'static str,
    severity: Severity,
    patterns: &'static [&'static str], // ANY match triggers
    applies_to: &'static [CommandKind],
    title: &'static str,
    explanation: &'static str,
    suggested: &'static [&'static str],
}
```

Engine: for each rule whose `applies_to` includes the kind, if any pattern is a substring of the log, emit a `Finding` capturing the matched line(s) as `evidence`. Findings render to the exact text and JSON shapes in `diagnostics-contract.md`.

**MVP rule set** (required): `msvc.missing_standard_headers`, `msvc.vsdevcmd_not_found`, `cmake.build_dir_missing`, `cmake.preset_missing`, `tool.cmake_not_found`, `tool.ctest_not_found`.
**Optional**: `qt.runtime_dll_missing`, `ctest.no_tests_matched`.

A fatal known-setup finding (e.g. missing headers, vsdevcmd not found) can raise the process exit code to **6** when it is the dominant cause; otherwise a failed-but-ran command is exit code **1** with diagnostics attached. The mapping is centralized in `error.rs`.

---

## 7. Test Strategy

Layered, with the pure core making most of it toolchain-free.

**Unit (no Qt/VS needed):**
- config parse + defaulting (`raw.rs`/`model.rs`).
- merge precedence (CLI > env > file > default) — table-driven.
- project root discovery (walk up to `CMakeLists.txt`) on a `tempfile` tree.
- planner output for configure/build/test/check — assert exact `program`/`args`.
- Windows command rendering + `quote_for_cmd` — golden strings.
- MSVC detection precedence — inject a fake filesystem/env, assert which source wins.
- diagnostics matching — feed canned logs, assert findings/codes.

**Snapshot:** `insta` snapshots of plan JSON and human renderings so contract drift is caught in review.

**Integration (`tests/`, fixture projects, no real build):**
- `doctor --json --no-probe` over a fixture → stable JSON.
- `plan check fake_test` → expected two-step plan.
- **non-Windows plan contains no MSVC bootstrap** (explicit invariant test).
- exit codes via `assert_cmd` (arg error→2, missing root→4).

**Windows smoke (`#[cfg(windows)]`, CI windows runner):**
- `qtflow doctor --json` runs outside a Developer Prompt.
- if a VS install is present, `qtflow plan check sample` and the quoting path.

Probes (`cmake --version`, `vswhere`) sit behind the `Probe` trait; unit/integration tests use a fake implementation, so CI without CMake still passes. Coverage gate target: planners, config merge, msvc precedence, diagnostics at ~full branch coverage.

---

## 8. GitHub Release Strategy

**Decision: `cargo-dist`-driven, tag-triggered release**, falling back to a hand-written matrix only if dist constraints bite.

- Trigger: pushing a `v*` tag runs the release workflow.
- Build targets (per architecture.md):
  - `x86_64-pc-windows-msvc`
  - `x86_64-unknown-linux-gnu`
  - `aarch64-apple-darwin`
  - `x86_64-apple-darwin`
- Artifacts: per-target archives (`.zip` for Windows, `.tar.gz` otherwise) containing the `qtflow` binary, plus a `SHA256SUMS` file.
- A `dist-manifest.json` (cargo-dist) lists assets — the **npm installer reads this** to pick the right binary/URL, so the two distribution channels share one source of truth.
- CI also runs `cargo test` on Windows + Linux before the release job; release is gated on green.
- Versioning: `Cargo.toml` version is the single source; tag must match (checked in CI).

---

## 9. npm Wrapper Strategy

**Decision: per-platform optional packages + a thin JS resolver. Avoid postinstall downloads as the default.**

Layout:
- Main package `qtflow` (or `@qtflow/cli`) with `"bin": { "qtflow": "bin/qtflow.js" }`.
- `optionalDependencies`: `@qtflow/cli-win32-x64`, `@qtflow/cli-linux-x64`, `@qtflow/cli-darwin-x64`, `@qtflow/cli-darwin-arm64`. Each is a tiny package shipping just the prebuilt binary with `os`/`cpu` fields so npm installs only the matching one.
- `bin/qtflow.js` resolves the binary from the installed platform package and `execFileSync`-spawns it, forwarding argv and exit code.

Why optionalDependencies over postinstall download:
- Works offline / behind proxies and in lockfile-pinned CI (no network at install time).
- npm's `os`/`cpu` filtering means no custom platform logic in postinstall.
- Reproducible: the binary version is pinned by the dependency version.

Fallback path: if no platform package resolves (unsupported target), `bin/qtflow.js` exits non-zero with a clear message pointing to the GitHub releases page and `cargo install` — "clearly fail with install instructions," per acceptance criteria. **No Rust toolchain is required to install.**

Publishing: a CI job after the GitHub release downloads the dist artifacts, packs each platform package + the wrapper, and `npm publish`es them with the same version.

---

## 10. MVP Milestone Plan

Sequenced so each milestone is independently testable; planning lands before anything touches the OS.

**M0 — Skeleton & contracts (no OS calls).**
clap surface, `GlobalArgs`, `Command` enum, `Invocation` bridge, error→exit-code table. `qtflow --help`/`--version` work. Exit codes 2/4 wired.

**M1 — Project + config core.**
`discover_root`, config discovery, raw→resolved merge with full precedence, inferred defaults. Unit tests for merge and discovery. `doctor --no-probe --json` renders project/config/profiles.

**M2 — Planning (the heart).**
Pure planners for configure/build/test/check; `CommandPlan` + serde; `plan <cmd>` and `--dry-run`; JSON matches contract. Snapshot tests. *Still no execution, no MSVC.* This already satisfies "testable without Qt or VS."

**M3 — Execution + non-Windows pass-through.**
`runner::execute_plan`, stream/capture output, exit-code propagation. Linux/macOS run real `cmake`/`ctest`. `--quiet`/`--verbose`.

**M4 — Windows MSVC bootstrap (first-class).**
`detect::msvc` precedence, `cmd.exe`+`VsDevCmd.bat` wrapping, `quote_for_cmd` with tests, `--no-msvc-bootstrap`/`--vsdevcmd`. `doctor` shows MSVC status; `--show-known-msvc`. Windows smoke test.

**M5 — Detection probes for doctor.**
`cmake --version`/`ctest --version` behind `Probe`, `CMakePresets.json` parsing + preset validation/warning, Qt hints. Full `doctor` output.

**M6 — Diagnostics engine.**
Rule table, post-failure analysis, text+JSON findings, exit code 6 for fatal setup issues. Required rule set + canned-log tests.

**M7 — Distribution.**
cargo-dist release workflow (4 targets, checksums); npm wrapper with optionalDependencies + resolver; README with install/examples/config schema/agent usage; ship the `qtflow-build-test` agent skill.

Optional / post-MVP (explicitly deferred): `@name` test presets, `qt.runtime_dll_missing` + `ctest.no_tests_matched` rules, `list-tests`, cached MSVC env injection, preset inference from `CMakePresets.json` when config absent, **qmake** (separate command group / product). None of these are required for Definition of Done.

---

## Constraint Compliance Summary

- **No qmake in MVP** — no qmake types or planners exist; future addition is an additive enum variant.
- **Do not replace CMake/CTest** — qtflow only ever shells out to `cmake`/`ctest`; it owns orchestration, not building.
- **Planning testable without Qt/VS** — planners are pure functions over config; M2 delivers full plan coverage with zero toolchain.
- **Windows/MSVC reliability first-class** — bootstrap is explicit serialized plan data, precedence-tested, with a dedicated quoting helper and smoke tests; failures map to specific exit codes + diagnostics.
- **Output useful for humans and agents** — one `CommandPlan`/finding model, two renderers (text + `--json`); stable exit codes; agent skill template ships in M7.
