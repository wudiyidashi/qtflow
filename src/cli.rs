use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "qtflow",
    version,
    about = "Standardize Qt/CMake configure, build, and CTest workflows."
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Args, Default)]
pub struct GlobalArgs {
    #[arg(
        long,
        global = true,
        value_name = "path",
        help = "Project root or any path inside the project"
    )]
    pub project: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        value_name = "path",
        help = "Explicit qtflow config file"
    )]
    pub config: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        value_name = "name",
        help = "Profile name; defaults to config default_profile or debug"
    )]
    pub profile: Option<String>,

    #[arg(long, global = true, help = "Emit JSON output where supported")]
    pub json: bool,

    #[arg(long, global = true, help = "Reduce output")]
    pub quiet: bool,

    #[arg(
        long,
        global = true,
        help = "Print detection trace and command details"
    )]
    pub verbose: bool,

    #[arg(
        long = "dry-run",
        global = true,
        help = "Print the command plan without executing"
    )]
    pub dry_run: bool,

    #[arg(long = "no-color", global = true, help = "Disable ANSI colors")]
    pub no_color: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Inspect the project, configuration, tools, and environment.")]
    Doctor(DoctorArgs),
    #[command(about = "Create qtflow config and agent integration files.")]
    Init(InitArgs),
    #[command(about = "Run the CMake configure step.")]
    Configure(ConfigureArgs),
    #[command(about = "Build one target or the default target.")]
    Build(BuildArgs),
    #[command(about = "Run CTest, optionally building a target first.")]
    Test(TestArgs),
    #[command(about = "Build the target, then run the matching CTest.")]
    Check(CheckArgs),
    #[command(about = "Bundle Qt runtime files next to a built executable.")]
    Deploy(DeployArgs),
    #[command(about = "Render a command plan without executing it.")]
    Plan {
        #[command(subcommand)]
        command: PlanCommand,
    },
}

#[derive(Debug, Clone, Args, Default)]
pub struct InitArgs {
    #[arg(
        long,
        value_enum,
        value_name = "claude|codex|cursor|all",
        help = "Install the qtflow-build-test skill for this agent; repeatable"
    )]
    pub agent: Vec<InitAgentArg>,

    #[arg(
        long,
        value_enum,
        value_name = "vs|qtcreator|cli|presets",
        help = "Force build-dir layout: vs, qtcreator, cli, or presets"
    )]
    pub layout: Option<InitLayoutArg>,

    #[arg(
        long = "build-dir-debug",
        value_name = "path",
        help = "Override the debug profile build directory"
    )]
    pub build_dir_debug: Option<PathBuf>,

    #[arg(
        long = "build-dir-release",
        value_name = "path",
        help = "Override the release profile build directory"
    )]
    pub build_dir_release: Option<PathBuf>,

    #[arg(
        long,
        help = "Install all supported agent skill files, not just detected agents"
    )]
    pub all: bool,

    #[arg(long, help = "Overwrite existing qtflow-managed files")]
    pub force: bool,

    #[arg(
        long,
        help = "Also install qtflow as a global Codex skill under $CODEX_HOME/skills (default ~/.codex/skills), discoverable by Codex across all projects."
    )]
    pub global: bool,

    #[arg(long = "no-config", help = "Do not create .qtflow.toml")]
    pub no_config: bool,

    #[arg(
        long = "config-only",
        help = "Create only .qtflow.toml and skip agent skill files"
    )]
    pub config_only: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum InitAgentArg {
    Claude,
    Codex,
    Cursor,
    All,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum InitLayoutArg {
    Vs,
    #[value(name = "qtcreator")]
    QtCreator,
    Cli,
    Presets,
}

#[derive(Debug, Clone, Args, Default)]
pub struct DoctorArgs {
    #[arg(
        long = "no-probe",
        help = "Do not execute cmake, ctest, or version probes"
    )]
    pub no_probe: bool,

    #[arg(
        long = "show-known-msvc",
        help = "Print known VsDevCmd candidate paths"
    )]
    pub show_known_msvc: bool,
}

#[derive(Debug, Clone, Args, Default)]
pub struct ConfigureArgs {
    #[arg(
        long,
        value_name = "name",
        help = "Override the configured CMake preset"
    )]
    pub preset: Option<String>,

    #[arg(
        long,
        value_name = "name",
        help = "CMake generator override when not using a preset"
    )]
    pub generator: Option<String>,

    #[arg(long, help = "Add CMake fresh configure behavior if supported")]
    pub fresh: bool,

    #[arg(
        long = "no-msvc-bootstrap",
        help = "Do not initialize the MSVC environment via VsDevCmd"
    )]
    pub no_msvc_bootstrap: bool,

    #[arg(long, value_name = "path", help = "Explicit VsDevCmd.bat path")]
    pub vsdevcmd: Option<PathBuf>,
}

#[derive(Debug, Clone, Args, Default)]
pub struct BuildArgs {
    #[arg(value_name = "target", help = "CMake target to build")]
    pub target: Option<String>,

    #[arg(
        long = "build-dir",
        value_name = "path",
        help = "Override build directory"
    )]
    pub build_dir: Option<PathBuf>,

    #[arg(long, value_name = "n", help = "Pass --parallel N to cmake --build")]
    pub parallel: Option<u32>,

    #[arg(
        long = "config-name",
        value_name = "name",
        help = "Build/test configuration for multi-config generators (e.g. Debug, Release)"
    )]
    pub config_name: Option<String>,

    #[arg(long, help = "Build the default/all target instead of a named target")]
    pub all: bool,

    #[arg(
        long = "no-msvc-bootstrap",
        help = "Do not initialize the MSVC environment via VsDevCmd"
    )]
    pub no_msvc_bootstrap: bool,

    #[arg(long, value_name = "path", help = "Explicit VsDevCmd.bat path")]
    pub vsdevcmd: Option<PathBuf>,
}

#[derive(Debug, Clone, Args, Default)]
pub struct TestArgs {
    #[arg(value_name = "regex", help = "CTest regex to run")]
    pub regex: Option<String>,

    #[arg(
        long = "build-target",
        value_name = "target",
        help = "Build this target before running CTest"
    )]
    pub build_target: Option<String>,

    #[arg(
        long = "build-dir",
        value_name = "path",
        help = "Override build directory"
    )]
    pub build_dir: Option<PathBuf>,

    #[arg(
        long = "config-name",
        value_name = "name",
        help = "Build/test configuration for multi-config generators (e.g. Debug, Release)"
    )]
    pub config_name: Option<String>,

    #[arg(
        long = "output-on-failure",
        overrides_with = "no_output_on_failure",
        help = "Show CTest output for failing tests"
    )]
    pub output_on_failure: bool,

    #[arg(
        long = "no-output-on-failure",
        overrides_with = "output_on_failure",
        help = "Disable CTest output-on-failure"
    )]
    pub no_output_on_failure: bool,

    #[arg(
        long = "ctest-arg",
        value_name = "arg",
        help = "Extra CTest argument; repeatable"
    )]
    pub ctest_arg: Vec<String>,

    #[arg(
        long,
        value_name = "n",
        help = "Build parallelism when --build-target is used"
    )]
    pub parallel: Option<u32>,

    #[arg(
        long = "no-msvc-bootstrap",
        help = "Do not initialize the MSVC environment via VsDevCmd"
    )]
    pub no_msvc_bootstrap: bool,

    #[arg(long, value_name = "path", help = "Explicit VsDevCmd.bat path")]
    pub vsdevcmd: Option<PathBuf>,
}

#[derive(Debug, Clone, Args, Default)]
pub struct CheckArgs {
    #[arg(value_name = "target", help = "CMake target to build before CTest")]
    pub target: String,

    #[arg(
        long = "test-regex",
        value_name = "regex",
        help = "CTest regex; defaults to the target"
    )]
    pub test_regex: Option<String>,

    #[arg(
        long = "build-dir",
        value_name = "path",
        help = "Override build directory"
    )]
    pub build_dir: Option<PathBuf>,

    #[arg(
        long = "config-name",
        value_name = "name",
        help = "Build/test configuration for multi-config generators (e.g. Debug, Release)"
    )]
    pub config_name: Option<String>,

    #[arg(long, value_name = "n", help = "Build parallelism")]
    pub parallel: Option<u32>,

    #[arg(
        long = "ctest-arg",
        value_name = "arg",
        help = "Extra CTest argument; repeatable"
    )]
    pub ctest_arg: Vec<String>,

    #[arg(
        long = "no-msvc-bootstrap",
        help = "Do not initialize the MSVC environment via VsDevCmd"
    )]
    pub no_msvc_bootstrap: bool,

    #[arg(long, value_name = "path", help = "Explicit VsDevCmd.bat path")]
    pub vsdevcmd: Option<PathBuf>,
}

#[derive(Debug, Clone, Args, Default)]
pub struct DeployArgs {
    #[arg(
        value_name = "target",
        help = "Build target whose executable should be deployed"
    )]
    pub target: Option<String>,

    #[arg(
        long,
        value_name = "path",
        help = "Explicit executable or .app bundle to deploy"
    )]
    pub exe: Option<PathBuf>,

    #[arg(
        long,
        conflicts_with = "debug",
        help = "Deploy release Qt runtime files"
    )]
    pub release: bool,

    #[arg(
        long,
        conflicts_with = "release",
        help = "Deploy debug Qt runtime files"
    )]
    pub debug: bool,

    #[arg(
        long,
        value_name = "dir",
        help = "QML source directory passed to the Qt deployment tool"
    )]
    pub qmldir: Option<PathBuf>,

    #[arg(
        long,
        value_name = "path",
        help = "Deployment output directory; defaults to next to the executable"
    )]
    pub dir: Option<PathBuf>,

    #[arg(
        long = "deploy-arg",
        value_name = "arg",
        allow_hyphen_values = true,
        help = "Extra argument passed through to the Qt deployment tool; repeatable"
    )]
    pub deploy_arg: Vec<String>,

    #[arg(
        long = "no-msvc-bootstrap",
        help = "Do not initialize the MSVC environment via VsDevCmd"
    )]
    pub no_msvc_bootstrap: bool,

    #[arg(long, value_name = "path", help = "Explicit VsDevCmd.bat path")]
    pub vsdevcmd: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum PlanCommand {
    #[command(about = "Render the CMake configure plan.")]
    Configure(ConfigureArgs),
    #[command(about = "Render the cmake --build plan.")]
    Build(BuildArgs),
    #[command(about = "Render the CTest plan.")]
    Test(TestArgs),
    #[command(about = "Render the build-then-CTest check plan.")]
    Check(CheckArgs),
    #[command(about = "Render the Qt deployment plan.")]
    Deploy(DeployArgs),
}
