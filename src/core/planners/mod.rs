use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::core::path::path_to_slash;
use crate::core::plan::{CommandStep, EnvironmentBootstrap};

pub mod build;
pub mod check;
pub mod configure;
pub mod deploy;
pub mod qmake_build;
pub mod qmake_check;
pub mod qmake_configure;
pub mod qmake_test;
pub mod test;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanContext {
    pub project_root: PathBuf,
    pub profile: String,
    pub active_profile: PlanProfile,
    pub tools: PlanTools,
    pub qmake: PlanQmake,
    pub msvc: PlanMsvc,
    pub command: PlanCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanProfile {
    pub preset: Option<String>,
    pub build_dir: PathBuf,
    pub generator: Option<String>,
    pub config_name: Option<String>,
    pub configure_args: Vec<String>,
    pub cache_variables: BTreeMap<String, String>,
    pub build_args: Vec<String>,
    pub ctest_args: Vec<String>,
    pub path_prepend: Vec<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanTools {
    pub cmake: String,
    pub ctest: String,
    pub ninja: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanQmake {
    pub qmake: String,
    pub spec: String,
    pub make: String,
    pub pro_file: PathBuf,
    pub config: QmakeBuildConfig,
    pub config_args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QmakeBuildConfig {
    Debug,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployBuildConfig {
    Debug,
    Release,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanMsvc {
    pub is_windows: bool,
    pub enabled: bool,
    pub no_bootstrap: bool,
    pub arch: String,
    pub host_arch: Option<String>,
    pub vsdevcmd: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanCommand {
    Configure(ConfigurePlanInputs),
    Build(BuildPlanInputs),
    Test(TestPlanInputs),
    Check(CheckPlanInputs),
    Deploy(DeployPlanInputs),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfigurePlanInputs {
    pub preset: Option<String>,
    pub generator: Option<String>,
    pub fresh: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildPlanInputs {
    pub target: Option<String>,
    pub build_dir: Option<PathBuf>,
    pub parallel: Option<u32>,
    pub config_name: Option<String>,
    pub all: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TestPlanInputs {
    pub regex: Option<String>,
    pub build_target: Option<String>,
    pub build_dir: Option<PathBuf>,
    pub config_name: Option<String>,
    pub output_on_failure: bool,
    pub no_output_on_failure: bool,
    pub ctest_arg: Vec<String>,
    pub parallel: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckPlanInputs {
    pub target: String,
    pub test_regex: Option<String>,
    pub build_dir: Option<PathBuf>,
    pub config_name: Option<String>,
    pub parallel: Option<u32>,
    pub ctest_arg: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployPlanInputs {
    pub tool: PathBuf,
    pub exe: PathBuf,
    pub config: DeployBuildConfig,
    pub qmldir: Option<PathBuf>,
    pub dir: Option<PathBuf>,
    pub deploy_args: Vec<String>,
    pub notes: Vec<String>,
}

impl Default for DeployPlanInputs {
    fn default() -> Self {
        Self {
            tool: PathBuf::from("windeployqt"),
            exe: PathBuf::from("/repo/out/build/debug/bin/app"),
            config: DeployBuildConfig::Debug,
            qmldir: None,
            dir: None,
            deploy_args: Vec::new(),
            notes: Vec::new(),
        }
    }
}

pub fn bootstrap(ctx: &PlanContext) -> Option<EnvironmentBootstrap> {
    if ctx.msvc.is_windows && ctx.msvc.enabled && !ctx.msvc.no_bootstrap {
        // TODO(M4): attach resolved VsDevCmd from detection when no explicit path is known.
        ctx.msvc
            .vsdevcmd
            .clone()
            .map(|vsdevcmd| EnvironmentBootstrap::Msvc {
                vsdevcmd,
                arch: ctx.msvc.arch.clone(),
                host_arch: ctx.msvc.host_arch.clone(),
            })
    } else {
        None
    }
}

fn step(ctx: &PlanContext, label: &str, program: String, args: Vec<String>) -> CommandStep {
    step_with_cwd(ctx, label, ctx.project_root.clone(), program, args)
}

fn step_with_cwd(
    ctx: &PlanContext,
    label: &str,
    cwd: PathBuf,
    program: String,
    args: Vec<String>,
) -> CommandStep {
    CommandStep {
        label: label.to_string(),
        cwd,
        program,
        args,
        env: ctx.active_profile.env.clone(),
        path_prepend: ctx.active_profile.path_prepend.clone(),
        bootstrap: bootstrap(ctx),
    }
}

fn build_step(
    ctx: &PlanContext,
    build_dir: &Path,
    target: Option<&str>,
    parallel: Option<u32>,
    config_name: Option<&str>,
) -> CommandStep {
    let mut args = vec!["--build".to_string(), path_to_slash(build_dir)];
    if let Some(config_name) = config_name {
        args.extend(["--config".to_string(), config_name.to_string()]);
    }
    if let Some(target) = target {
        args.extend(["--target".to_string(), target.to_string()]);
    }
    if let Some(parallel) = parallel {
        args.extend(["--parallel".to_string(), parallel.to_string()]);
    }
    args.extend(ctx.active_profile.build_args.clone());

    step(ctx, "build", ctx.tools.cmake.clone(), args)
}

fn test_step(
    ctx: &PlanContext,
    build_dir: &Path,
    regex: Option<&str>,
    config_name: Option<&str>,
    include_output_on_failure: bool,
    extra_ctest_args: &[String],
) -> CommandStep {
    let mut args = vec!["--test-dir".to_string(), path_to_slash(build_dir)];
    if let Some(config_name) = config_name {
        args.extend(["-C".to_string(), config_name.to_string()]);
    }
    if let Some(regex) = regex {
        args.extend(["-R".to_string(), regex.to_string()]);
    }
    if include_output_on_failure {
        args.push("--output-on-failure".to_string());
    }
    args.extend(extra_ctest_args.iter().cloned());
    append_profile_ctest_args(&mut args, &ctx.active_profile.ctest_args);

    step(ctx, "test", ctx.tools.ctest.clone(), args)
}

fn append_profile_ctest_args(args: &mut Vec<String>, profile_ctest_args: &[String]) {
    // M2 owns the standard output-on-failure flag in the planner so CLI disablement is reliable.
    args.extend(
        profile_ctest_args
            .iter()
            .filter(|arg| arg.as_str() != "--output-on-failure")
            .cloned(),
    );
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub fn context(command: PlanCommand) -> PlanContext {
        PlanContext {
            project_root: PathBuf::from("/repo"),
            profile: "debug".to_string(),
            active_profile: PlanProfile {
                preset: Some("Qt-Debug".to_string()),
                build_dir: PathBuf::from("/repo/out/build/debug"),
                generator: None,
                config_name: None,
                configure_args: Vec::new(),
                cache_variables: BTreeMap::new(),
                build_args: Vec::new(),
                ctest_args: Vec::new(),
                path_prepend: Vec::new(),
                env: BTreeMap::new(),
            },
            tools: PlanTools {
                cmake: "cmake".to_string(),
                ctest: "ctest".to_string(),
                ninja: None,
            },
            qmake: PlanQmake {
                qmake: "qmake".to_string(),
                spec: "linux-g++".to_string(),
                make: "make".to_string(),
                pro_file: PathBuf::from("/repo/app.pro"),
                config: QmakeBuildConfig::Debug,
                config_args: Vec::new(),
            },
            msvc: PlanMsvc {
                is_windows: false,
                enabled: true,
                no_bootstrap: false,
                arch: "x64".to_string(),
                host_arch: None,
                vsdevcmd: None,
            },
            command,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_none_for_non_windows_context() {
        let mut ctx = test_support::context(PlanCommand::Build(BuildPlanInputs::default()));
        ctx.msvc.is_windows = false;
        ctx.msvc.vsdevcmd = Some(PathBuf::from("C:/VsDevCmd.bat"));

        assert_eq!(bootstrap(&ctx), None);
    }

    #[test]
    fn bootstrap_some_for_windows_enabled_known_vsdevcmd() {
        let mut ctx = test_support::context(PlanCommand::Build(BuildPlanInputs::default()));
        ctx.msvc.is_windows = true;
        ctx.msvc.enabled = true;
        ctx.msvc.no_bootstrap = false;
        ctx.msvc.vsdevcmd = Some(PathBuf::from("C:/VsDevCmd.bat"));

        assert_eq!(
            bootstrap(&ctx),
            Some(EnvironmentBootstrap::Msvc {
                vsdevcmd: PathBuf::from("C:/VsDevCmd.bat"),
                arch: "x64".to_string(),
                host_arch: None
            })
        );
    }

    #[test]
    fn bootstrap_none_when_disabled_or_flag_disabled() {
        let mut ctx = test_support::context(PlanCommand::Build(BuildPlanInputs::default()));
        ctx.msvc.is_windows = true;
        ctx.msvc.vsdevcmd = Some(PathBuf::from("C:/VsDevCmd.bat"));

        ctx.msvc.enabled = false;
        assert_eq!(bootstrap(&ctx), None);

        ctx.msvc.enabled = true;
        ctx.msvc.no_bootstrap = true;
        assert_eq!(bootstrap(&ctx), None);
    }

    #[test]
    fn bootstrap_none_for_windows_without_known_vsdevcmd() {
        let mut ctx = test_support::context(PlanCommand::Build(BuildPlanInputs::default()));
        ctx.msvc.is_windows = true;
        ctx.msvc.enabled = true;
        ctx.msvc.vsdevcmd = None;

        assert_eq!(bootstrap(&ctx), None);
    }
}
