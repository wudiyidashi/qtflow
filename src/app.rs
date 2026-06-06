use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cli::{
    BuildArgs, CheckArgs, Cli, Command, ConfigureArgs, DoctorArgs, GlobalArgs, InitAgentArg,
    InitArgs, InitLayoutArg, PlanCommand, TestArgs,
};
use crate::core::config::merge::{resolve, ConfigOverrides};
use crate::core::config::model::{BuildSystem, ConfigSource, Profile, ResolvedConfig};
use crate::core::config::raw::RawConfig;
use crate::core::detect::builddir::{
    classify, discover_build_dirs, AmbiguityWarning, DiscoverOptions, Provenance,
};
use crate::core::detect::msvc::{
    known_vsdevcmd_candidates, resolve_vsdevcmd, MsvcResolveInput, VsDevCmdResolution,
    VsDevCmdSource,
};
use crate::core::detect::{cmake, ctest, qmake, qt, Probe, SystemProbe};
use crate::core::diagnostics::report::{self, DiagnosticReport};
use crate::core::diagnostics::{
    exit_code_override, CommandKind, DiagnosticContext, Engine, Platform,
};
use crate::core::init::{
    apply_plan as apply_init_plan, detect_agents, init_json_report,
    plan_global_codex_skill_actions, resolve_codex_skills_root, Agent, AgentSelection, InitAction,
    InitConfigInputs, InitLayout, InitOptions, RealInitFileSystem,
};
use crate::core::path::{path_to_slash, serialize_optional_path, serialize_path};
use crate::core::plan::{CommandPlan, EnvironmentBootstrap};
use crate::core::planners::{
    self, BuildPlanInputs, CheckPlanInputs, ConfigurePlanInputs, PlanContext, PlanMsvc,
    PlanProfile, PlanQmake, PlanTools, QmakeBuildConfig, TestPlanInputs,
};
use crate::core::project::{
    discover_root, discover_root_with_preference, locate_project_config, BuildSystemPreference,
    ProjectContext, ProjectKind,
};
use crate::core::runner::shell::{render_command_display, run_vswhere};
use crate::core::runner::{execute_plan, RunOptions};
use crate::error::QtflowError;

pub fn run(cli: Cli) -> Result<(), QtflowError> {
    let invocation = Invocation::from(cli);
    dispatch(invocation)
}

#[derive(Debug, Clone)]
pub struct Invocation {
    pub global: GlobalInvocation,
    pub command: InvocationCommand,
}

#[derive(Debug, Clone, Default)]
pub struct GlobalInvocation {
    pub project: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub profile: Option<String>,
    pub json: bool,
    pub quiet: bool,
    pub verbose: bool,
    pub dry_run: bool,
    pub no_color: bool,
}

#[derive(Debug, Clone)]
pub enum InvocationCommand {
    Doctor(DoctorInvocation),
    Init(InitInvocation),
    Configure(ConfigureInvocation),
    Build(BuildInvocation),
    Test(TestInvocation),
    Check(CheckInvocation),
    Plan(PlanInvocation),
}

#[derive(Debug, Clone, Default)]
pub struct InitInvocation {
    pub agents: Vec<AgentSelection>,
    pub all: bool,
    pub force: bool,
    pub global: bool,
    pub no_config: bool,
    pub config_only: bool,
    pub layout: Option<InitLayout>,
    pub build_dir_debug: Option<PathBuf>,
    pub build_dir_release: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct DoctorInvocation {
    pub no_probe: bool,
    pub show_known_msvc: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigureInvocation {
    pub preset: Option<String>,
    pub generator: Option<String>,
    pub fresh: bool,
    pub no_msvc_bootstrap: bool,
    pub vsdevcmd: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct BuildInvocation {
    pub target: Option<String>,
    pub build_dir: Option<PathBuf>,
    pub parallel: Option<u32>,
    pub config_name: Option<String>,
    pub all: bool,
    pub no_msvc_bootstrap: bool,
    pub vsdevcmd: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct TestInvocation {
    pub regex: Option<String>,
    pub build_target: Option<String>,
    pub build_dir: Option<PathBuf>,
    pub config_name: Option<String>,
    pub output_on_failure: bool,
    pub no_output_on_failure: bool,
    pub ctest_arg: Vec<String>,
    pub parallel: Option<u32>,
    pub no_msvc_bootstrap: bool,
    pub vsdevcmd: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct CheckInvocation {
    pub target: String,
    pub test_regex: Option<String>,
    pub build_dir: Option<PathBuf>,
    pub config_name: Option<String>,
    pub parallel: Option<u32>,
    pub ctest_arg: Vec<String>,
    pub no_msvc_bootstrap: bool,
    pub vsdevcmd: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum PlanInvocation {
    Configure(ConfigureInvocation),
    Build(BuildInvocation),
    Test(TestInvocation),
    Check(CheckInvocation),
}

impl From<Cli> for Invocation {
    fn from(cli: Cli) -> Self {
        Self {
            global: cli.global.into(),
            command: cli.command.into(),
        }
    }
}

impl From<GlobalArgs> for GlobalInvocation {
    fn from(args: GlobalArgs) -> Self {
        Self {
            project: args.project,
            config: args.config,
            profile: args.profile,
            json: args.json,
            quiet: args.quiet,
            verbose: args.verbose,
            dry_run: args.dry_run,
            no_color: args.no_color,
        }
    }
}

impl From<Command> for InvocationCommand {
    fn from(command: Command) -> Self {
        match command {
            Command::Doctor(args) => Self::Doctor(args.into()),
            Command::Init(args) => Self::Init(args.into()),
            Command::Configure(args) => Self::Configure(args.into()),
            Command::Build(args) => Self::Build(args.into()),
            Command::Test(args) => Self::Test(args.into()),
            Command::Check(args) => Self::Check(args.into()),
            Command::Plan { command } => Self::Plan(command.into()),
        }
    }
}

impl From<InitArgs> for InitInvocation {
    fn from(args: InitArgs) -> Self {
        Self {
            agents: args.agent.into_iter().map(AgentSelection::from).collect(),
            all: args.all,
            force: args.force,
            global: args.global,
            no_config: args.no_config,
            config_only: args.config_only,
            layout: args.layout.map(InitLayout::from),
            build_dir_debug: args.build_dir_debug,
            build_dir_release: args.build_dir_release,
        }
    }
}

impl From<InitAgentArg> for AgentSelection {
    fn from(agent: InitAgentArg) -> Self {
        match agent {
            InitAgentArg::Claude => Self::Agent(Agent::Claude),
            InitAgentArg::Codex => Self::Agent(Agent::Codex),
            InitAgentArg::Cursor => Self::Agent(Agent::Cursor),
            InitAgentArg::All => Self::All,
        }
    }
}

impl From<InitLayoutArg> for InitLayout {
    fn from(layout: InitLayoutArg) -> Self {
        match layout {
            InitLayoutArg::Vs => Self::Vs,
            InitLayoutArg::QtCreator => Self::QtCreator,
            InitLayoutArg::Cli => Self::Cli,
            InitLayoutArg::Presets => Self::Presets,
        }
    }
}

impl From<DoctorArgs> for DoctorInvocation {
    fn from(args: DoctorArgs) -> Self {
        Self {
            no_probe: args.no_probe,
            show_known_msvc: args.show_known_msvc,
        }
    }
}

impl From<ConfigureArgs> for ConfigureInvocation {
    fn from(args: ConfigureArgs) -> Self {
        Self {
            preset: args.preset,
            generator: args.generator,
            fresh: args.fresh,
            no_msvc_bootstrap: args.no_msvc_bootstrap,
            vsdevcmd: args.vsdevcmd,
        }
    }
}

impl From<BuildArgs> for BuildInvocation {
    fn from(args: BuildArgs) -> Self {
        Self {
            target: args.target,
            build_dir: args.build_dir,
            parallel: args.parallel,
            config_name: args.config_name,
            all: args.all,
            no_msvc_bootstrap: args.no_msvc_bootstrap,
            vsdevcmd: args.vsdevcmd,
        }
    }
}

impl From<TestArgs> for TestInvocation {
    fn from(args: TestArgs) -> Self {
        Self {
            regex: args.regex,
            build_target: args.build_target,
            build_dir: args.build_dir,
            config_name: args.config_name,
            output_on_failure: args.output_on_failure,
            no_output_on_failure: args.no_output_on_failure,
            ctest_arg: args.ctest_arg,
            parallel: args.parallel,
            no_msvc_bootstrap: args.no_msvc_bootstrap,
            vsdevcmd: args.vsdevcmd,
        }
    }
}

impl From<CheckArgs> for CheckInvocation {
    fn from(args: CheckArgs) -> Self {
        Self {
            target: args.target,
            test_regex: args.test_regex,
            build_dir: args.build_dir,
            config_name: args.config_name,
            parallel: args.parallel,
            ctest_arg: args.ctest_arg,
            no_msvc_bootstrap: args.no_msvc_bootstrap,
            vsdevcmd: args.vsdevcmd,
        }
    }
}

impl From<PlanCommand> for PlanInvocation {
    fn from(command: PlanCommand) -> Self {
        match command {
            PlanCommand::Configure(args) => Self::Configure(args.into()),
            PlanCommand::Build(args) => Self::Build(args.into()),
            PlanCommand::Test(args) => Self::Test(args.into()),
            PlanCommand::Check(args) => Self::Check(args.into()),
        }
    }
}

fn dispatch(invocation: Invocation) -> Result<(), QtflowError> {
    match &invocation.command {
        InvocationCommand::Doctor(args) => run_doctor(&invocation.global, args),
        InvocationCommand::Init(args) => run_init(&invocation.global, args),
        InvocationCommand::Configure(args) => run_planned_command(
            &invocation.global,
            PlanInvocation::Configure(args.clone()),
            "configure",
            false,
        ),
        InvocationCommand::Build(args) => run_planned_command(
            &invocation.global,
            PlanInvocation::Build(args.clone()),
            "build",
            false,
        ),
        InvocationCommand::Test(args) => run_planned_command(
            &invocation.global,
            PlanInvocation::Test(args.clone()),
            "test",
            false,
        ),
        InvocationCommand::Check(args) => run_planned_command(
            &invocation.global,
            PlanInvocation::Check(args.clone()),
            "check",
            false,
        ),
        InvocationCommand::Plan(args) => {
            run_planned_command(&invocation.global, args.clone(), "plan", true)
        }
    }
}

fn run_planned_command(
    global: &GlobalInvocation,
    invocation: PlanInvocation,
    command_name: &str,
    force_render: bool,
) -> Result<(), QtflowError> {
    let render_only = force_render || global.dry_run;
    let ctx = resolve_context(global, plan_overrides(&invocation))?;
    let plan = build_command_plan(&ctx, &invocation)?;
    let bootstrap = match resolve_execution_bootstrap(&ctx, &invocation, render_only) {
        Ok(bootstrap) => bootstrap,
        Err(QtflowError::EnvironmentBootstrap(message)) => {
            let step_label = plan
                .steps
                .first()
                .map(|step| step.label.as_str())
                .unwrap_or(command_name);
            return report_failed_step(global, &ctx, command_name, step_label, 1, &message, false);
        }
        Err(err) => return Err(err),
    };
    let plan = attach_execution_bootstrap(plan, bootstrap);

    if render_only {
        render_plan(&plan, global)?;
        Ok(())
    } else {
        if !global.quiet {
            eprintln!("qtflow: running {command_name}");
        }
        let outcome = execute_plan(
            &plan,
            &RunOptions {
                quiet: global.quiet || global.json,
                verbose: global.verbose,
                max_log_bytes: ctx.config.diagnostics.max_log_bytes,
            },
        )?;
        if outcome.last_exit_code == 0 {
            Ok(())
        } else {
            let message = format!(
                "{command_name} failed after {} step(s) with exit code {}",
                outcome.steps_run, outcome.last_exit_code
            );
            if let Some(failure) = &outcome.failure {
                report_failed_step(
                    global,
                    &ctx,
                    command_name,
                    &failure.step_label,
                    outcome.last_exit_code,
                    &failure.combined_log,
                    failure.bootstrap_used,
                )
            } else {
                report_rendered_failure(global, &message, outcome.last_exit_code, Vec::new())
            }
        }
    }
}

fn report_failed_step(
    global: &GlobalInvocation,
    ctx: &AppContext,
    command_name: &str,
    step_label: &str,
    child_exit_code: i32,
    combined_log: &str,
    bootstrap_used: bool,
) -> Result<(), QtflowError> {
    let findings = if ctx.config.diagnostics.enabled {
        CommandKind::from_label(step_label)
            .map(|command_kind| {
                Engine::new(ctx.config.diagnostics.max_log_bytes).analyze(&DiagnosticContext {
                    exit_code: child_exit_code,
                    command_kind,
                    combined_log,
                    platform: Platform::current(),
                    bootstrap_used,
                })
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let exit_code = exit_code_override(&findings, bootstrap_used).unwrap_or(1);
    let message = format!("{command_name} failed with exit code {child_exit_code}");

    report_rendered_failure(global, &message, exit_code, findings)
}

fn report_rendered_failure(
    global: &GlobalInvocation,
    message: &str,
    exit_code: i32,
    findings: Vec<crate::core::diagnostics::Finding>,
) -> Result<(), QtflowError> {
    if global.json {
        println!(
            "{}",
            report::render_json(&DiagnosticReport {
                exit_code,
                diagnostics: findings,
            })
            .map_err(|err| QtflowError::ConfigOrArg(err.to_string()))?
        );
    } else if !global.quiet {
        eprint!("{}", report::render_text(message, &findings));
    }

    Err(QtflowError::ReportedFailure { exit_code })
}

fn build_command_plan(
    ctx: &AppContext,
    invocation: &PlanInvocation,
) -> Result<CommandPlan, QtflowError> {
    let plan_ctx = build_plan_context(ctx, invocation);
    match ctx.project.kind {
        ProjectKind::Cmake => match &plan_ctx.command {
            planners::PlanCommand::Configure(_) => planners::configure::plan(&plan_ctx),
            planners::PlanCommand::Build(_) => planners::build::plan(&plan_ctx),
            planners::PlanCommand::Test(_) => planners::test::plan(&plan_ctx),
            planners::PlanCommand::Check(_) => planners::check::plan(&plan_ctx),
        },
        ProjectKind::Qmake => match &plan_ctx.command {
            planners::PlanCommand::Configure(_) => planners::qmake_configure::plan(&plan_ctx),
            planners::PlanCommand::Build(_) => planners::qmake_build::plan(&plan_ctx),
            planners::PlanCommand::Test(_) | planners::PlanCommand::Check(_) => {
                Err(qmake_test_check_error())
            }
        },
    }
}

fn build_plan_context(ctx: &AppContext, invocation: &PlanInvocation) -> PlanContext {
    let active_profile = ctx
        .config
        .profiles
        .get(&ctx.config.active_profile)
        .expect("active profile is inserted during resolution");

    PlanContext {
        project_root: ctx.project.root.clone(),
        profile: ctx.config.active_profile.clone(),
        active_profile: PlanProfile::from(active_profile),
        tools: PlanTools {
            cmake: ctx.config.tools.cmake.clone(),
            ctest: ctx.config.tools.ctest.clone(),
        },
        qmake: PlanQmake {
            qmake: qmake_program_for_plan(ctx),
            spec: qmake_spec_for_plan(ctx),
            make: qmake_make_for_plan(ctx),
            pro_file: qmake_pro_file_for_plan(ctx),
            config: qmake_config_for_profile(
                &ctx.config.active_profile,
                active_profile.config_name.as_deref(),
            ),
            config_args: ctx.config.qmake.config_args.clone(),
        },
        msvc: PlanMsvc {
            is_windows: cfg!(windows),
            enabled: ctx.config.msvc.enabled,
            no_bootstrap: invocation.no_msvc_bootstrap(),
            arch: ctx.config.msvc.arch.clone(),
            host_arch: ctx.config.msvc.host_arch.clone(),
            vsdevcmd: None,
        },
        command: invocation
            .plan_command_with_profile_config_name(active_profile.config_name.as_deref()),
    }
}

fn qmake_test_check_error() -> QtflowError {
    QtflowError::ConfigOrArg(
        "qmake test/check not yet supported (coming in next phase)".to_string(),
    )
}

fn qmake_program_for_plan(ctx: &AppContext) -> String {
    let input = qmake::QmakeResolveInput::real(ctx.config.qmake.qmake.clone(), ctx.env.clone());
    qmake::resolve_qmake(&input)
        .path
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .or_else(|| ctx.env.get("QTFLOW_QMAKE").cloned())
        .or_else(|| ctx.config.qmake.qmake.clone())
        .unwrap_or_else(|| "qmake".to_string())
}

fn qmake_spec_for_plan(ctx: &AppContext) -> String {
    ctx.config
        .qmake
        .spec
        .clone()
        .unwrap_or_else(|| qmake::default_spec(cfg!(windows), ctx.config.msvc.enabled).to_string())
}

fn qmake_make_for_plan(ctx: &AppContext) -> String {
    let spec = qmake_spec_for_plan(ctx);
    let input =
        qmake::MakeToolResolveInput::real(ctx.config.qmake.make.clone(), spec, cfg!(windows));
    qmake::resolve_make_tool(&input).tool
}

fn qmake_pro_file_for_plan(ctx: &AppContext) -> PathBuf {
    ctx.config
        .qmake
        .pro_file
        .clone()
        .unwrap_or_else(|| ctx.project.project_file.clone())
}

fn qmake_config_for_profile(profile: &str, profile_config_name: Option<&str>) -> QmakeBuildConfig {
    // qmake only needs debug/release here. A profile config_name of Debug/Release acts as
    // a simple override; otherwise the profile name is used.
    let value = profile_config_name.unwrap_or(profile).to_lowercase();
    if value.contains("release") {
        QmakeBuildConfig::Release
    } else {
        QmakeBuildConfig::Debug
    }
}

fn resolve_execution_bootstrap(
    ctx: &AppContext,
    invocation: &PlanInvocation,
    render_only: bool,
) -> Result<Option<EnvironmentBootstrap>, QtflowError> {
    if !cfg!(windows) || !ctx.config.msvc.enabled || invocation.no_msvc_bootstrap() {
        return Ok(None);
    }

    let input = MsvcResolveInput::real(
        invocation.vsdevcmd(),
        ctx.env.clone(),
        config_vsdevcmd_path(ctx),
        Some(&run_vswhere),
    );

    match resolve_vsdevcmd(&input) {
        VsDevCmdResolution::Found { path, .. } => Ok(Some(EnvironmentBootstrap::Msvc {
            vsdevcmd: path,
            arch: ctx.config.msvc.arch.clone(),
            host_arch: ctx.config.msvc.host_arch.clone(),
        })),
        VsDevCmdResolution::NotFound { searched } if render_only => {
            let _ = searched;
            Ok(None)
        }
        VsDevCmdResolution::NotFound { searched } => {
            Err(QtflowError::EnvironmentBootstrap(format!(
                "VsDevCmd.bat was not found; searched {}",
                display_sources(&searched)
            )))
        }
    }
}

fn attach_execution_bootstrap(
    mut plan: CommandPlan,
    bootstrap: Option<EnvironmentBootstrap>,
) -> CommandPlan {
    if let Some(bootstrap) = bootstrap {
        for step in &mut plan.steps {
            step.bootstrap = Some(bootstrap.clone());
        }
    }
    plan
}

fn config_vsdevcmd_path(ctx: &AppContext) -> Option<PathBuf> {
    let env_path = ctx
        .env
        .get("QTFLOW_VSDEVCMD_BAT")
        .or_else(|| ctx.env.get("VSDEVCMD_BAT"))
        .map(PathBuf::from);

    if ctx.config.msvc.vsdevcmd == env_path {
        None
    } else {
        ctx.config.msvc.vsdevcmd.clone()
    }
}

fn display_sources(sources: &[VsDevCmdSource]) -> String {
    sources
        .iter()
        .map(|source| format!("{source:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl From<&Profile> for PlanProfile {
    fn from(profile: &Profile) -> Self {
        Self {
            preset: profile.preset.clone(),
            build_dir: profile.build_dir.clone(),
            generator: profile.generator.clone(),
            config_name: profile.config_name.clone(),
            configure_args: profile.configure_args.clone(),
            build_args: profile.build_args.clone(),
            ctest_args: profile.ctest_args.clone(),
            env: profile.env.clone(),
        }
    }
}

impl PlanInvocation {
    fn no_msvc_bootstrap(&self) -> bool {
        match self {
            Self::Configure(args) => args.no_msvc_bootstrap,
            Self::Build(args) => args.no_msvc_bootstrap,
            Self::Test(args) => args.no_msvc_bootstrap,
            Self::Check(args) => args.no_msvc_bootstrap,
        }
    }

    fn vsdevcmd(&self) -> Option<PathBuf> {
        match self {
            Self::Configure(args) => args.vsdevcmd.clone(),
            Self::Build(args) => args.vsdevcmd.clone(),
            Self::Test(args) => args.vsdevcmd.clone(),
            Self::Check(args) => args.vsdevcmd.clone(),
        }
    }

    fn plan_command_with_profile_config_name(
        &self,
        profile_config_name: Option<&str>,
    ) -> planners::PlanCommand {
        match self {
            Self::Configure(args) => planners::PlanCommand::Configure(ConfigurePlanInputs {
                preset: args.preset.clone(),
                generator: args.generator.clone(),
                fresh: args.fresh,
            }),
            Self::Build(args) => planners::PlanCommand::Build(BuildPlanInputs {
                target: args.target.clone(),
                build_dir: args.build_dir.clone(),
                parallel: args.parallel,
                config_name: effective_config_name(
                    args.config_name.as_deref(),
                    profile_config_name,
                ),
                all: args.all,
            }),
            Self::Test(args) => planners::PlanCommand::Test(TestPlanInputs {
                regex: args.regex.clone(),
                build_target: args.build_target.clone(),
                build_dir: args.build_dir.clone(),
                config_name: effective_config_name(
                    args.config_name.as_deref(),
                    profile_config_name,
                ),
                output_on_failure: args.output_on_failure,
                no_output_on_failure: args.no_output_on_failure,
                ctest_arg: args.ctest_arg.clone(),
                parallel: args.parallel,
            }),
            Self::Check(args) => planners::PlanCommand::Check(CheckPlanInputs {
                target: args.target.clone(),
                test_regex: args.test_regex.clone(),
                build_dir: args.build_dir.clone(),
                config_name: effective_config_name(
                    args.config_name.as_deref(),
                    profile_config_name,
                ),
                parallel: args.parallel,
                ctest_arg: args.ctest_arg.clone(),
            }),
        }
    }
}

fn effective_config_name(cli: Option<&str>, profile: Option<&str>) -> Option<String> {
    cli.or(profile).map(str::to_string)
}

fn render_plan(plan: &CommandPlan, global: &GlobalInvocation) -> Result<(), QtflowError> {
    if global.json {
        println!(
            "{}",
            serde_json::to_string_pretty(plan)
                .map_err(|err| QtflowError::ConfigOrArg(err.to_string()))?
        );
    } else {
        print_plan_text(plan);
    }

    Ok(())
}

fn print_plan_text(plan: &CommandPlan) {
    println!(
        "Plan for profile '{}' at {}",
        plan.profile,
        path_to_slash(&plan.project_root)
    );
    for step in &plan.steps {
        let command = render_command_display(step);
        match &step.bootstrap {
            Some(EnvironmentBootstrap::Msvc {
                vsdevcmd,
                arch,
                host_arch,
            }) => {
                let host = host_arch
                    .as_deref()
                    .map(|host_arch| format!(", host={host_arch}"))
                    .unwrap_or_default();
                println!(
                    "{}: {}  [msvc: {} arch={}{}]",
                    step.label,
                    command,
                    path_to_slash(vsdevcmd),
                    arch,
                    host
                );
            }
            None => println!("{}: {}", step.label, command),
        }
    }
}

fn run_init(global: &GlobalInvocation, args: &InitInvocation) -> Result<(), QtflowError> {
    let start = global.project.clone().unwrap_or(
        std::env::current_dir().map_err(|err| QtflowError::ConfigOrArg(err.to_string()))?,
    );
    let project = match discover_root(&start) {
        Ok(project) => Some(project),
        Err(QtflowError::ProjectRootNotFound { .. }) if args.global => None,
        Err(err) => return Err(err),
    };
    let mut fs = RealInitFileSystem;
    let detected = project
        .as_ref()
        .map(|project| detect_agents(&project.root, &fs))
        .unwrap_or_default();
    let opts = InitOptions {
        agents: args.agents.clone(),
        all: args.all,
        force: args.force,
        no_config: args.no_config,
        config_only: args.config_only,
        dry_run: global.dry_run,
        layout: args.layout,
        build_dir_debug: args.build_dir_debug.clone(),
        build_dir_release: args.build_dir_release.clone(),
    };
    let mut plan = if let Some(project) = &project {
        let presets = cmake::list_preset_infos(&project.root).unwrap_or_default();
        let discovered_build_dirs = discover_build_dirs(&project.root, &DiscoverOptions::default());
        let inputs = InitConfigInputs {
            discovered_build_dirs,
            presets,
        };
        crate::core::init::plan_actions(&project.root, &opts, &detected, &fs, &inputs)
            .map_err(|err| QtflowError::ConfigOrArg(err.to_string()))?
    } else {
        crate::core::init::InitPlan {
            project_root: start.clone(),
            detected_agents: detected.clone(),
            actions: Vec::new(),
            warnings: vec![format!(
                "project root not found from {}; skipped repo-scoped init actions",
                path_to_slash(&start)
            )],
        }
    };

    if args.global {
        let env = collect_env();
        let skills_root = resolve_codex_skills_root(&env, None)
            .map_err(|err| QtflowError::ConfigOrArg(err.to_string()))?;
        plan.actions.extend(plan_global_codex_skill_actions(
            &skills_root,
            args.force,
            global.dry_run,
            &fs,
        ));
    }

    if !global.dry_run {
        let applied = apply_init_plan(&plan, &mut fs).map_err(|err| {
            QtflowError::ConfigOrArg(format!("failed to apply init actions: {err}"))
        })?;
        plan.actions = applied;
    }

    if global.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&init_json_report(&plan))
                .map_err(|err| QtflowError::ConfigOrArg(err.to_string()))?
        );
    } else if !global.quiet {
        print_init_text(&plan.detected_agents, &plan.actions, &plan.warnings);
        if should_print_agent_hint(args, &plan.detected_agents) {
            println!("hint: no agents detected; use --agent claude, --agent codex, --agent cursor, or --all");
        }
        if args.global && !global.dry_run && global_install_changed(&plan.actions) {
            println!("Restart Codex to pick up the new skill.");
        }
    }

    Ok(())
}

fn should_print_agent_hint(
    args: &InitInvocation,
    detected: &crate::core::init::DetectedAgents,
) -> bool {
    !args.all
        && !args.global
        && !args.config_only
        && args.agents.is_empty()
        && !detected.claude
        && !detected.codex
        && !detected.cursor
}

fn print_init_text(
    detected: &crate::core::init::DetectedAgents,
    actions: &[InitAction],
    warnings: &[String],
) {
    println!("Detected agents: {}", detected_agents_display(detected));
    if actions.is_empty() {
        println!("No init actions selected.");
    }
    for action in actions {
        let target = action
            .agent_name()
            .map(|agent| format!("{agent} "))
            .unwrap_or_default();
        println!(
            "{}{}: {}",
            target,
            action.status.human_name(),
            path_to_slash(&action.path)
        );
    }
    for warning in warnings {
        println!("warning: {warning}");
    }
}

fn global_install_changed(actions: &[InitAction]) -> bool {
    actions.iter().any(|action| {
        action.agent_name() == Some(crate::core::init::GLOBAL_CODEX_AGENT_LABEL)
            && matches!(
                action.status,
                crate::core::init::InitStatus::Create | crate::core::init::InitStatus::Overwrite
            )
    })
}

fn detected_agents_display(detected: &crate::core::init::DetectedAgents) -> String {
    let mut names = Vec::new();
    if detected.claude {
        names.push("claude");
    }
    if detected.codex {
        names.push("codex");
    }
    if detected.cursor {
        names.push("cursor");
    }
    if names.is_empty() {
        "none".to_string()
    } else {
        names.join(", ")
    }
}

fn run_doctor(global: &GlobalInvocation, args: &DoctorInvocation) -> Result<(), QtflowError> {
    let ctx = match resolve_context(global, ConfigOverrides::default()) {
        Ok(ctx) => ctx,
        Err(QtflowError::ProjectRootNotFound { .. }) if args.show_known_msvc => {
            return run_known_msvc_only(global);
        }
        Err(err) => return Err(err),
    };
    let report = DoctorReport::from_context(&ctx, args, &SystemProbe);

    if global.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|err| QtflowError::ConfigOrArg(err.to_string()))?
        );
    } else if !global.quiet {
        print_doctor_text(&report);
    }

    Ok(())
}

fn run_known_msvc_only(global: &GlobalInvocation) -> Result<(), QtflowError> {
    let env = collect_env();
    let report = KnownMsvcReport {
        show_known_msvc: true,
        known_msvc_candidates: known_msvc_candidate_strings(&env),
    };

    if global.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|err| QtflowError::ConfigOrArg(err.to_string()))?
        );
    } else if !global.quiet {
        print_known_msvc_text(&report.known_msvc_candidates);
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct AppContext {
    project: ProjectContext,
    config: ResolvedConfig,
    env: BTreeMap<String, String>,
}

fn resolve_context(
    global: &GlobalInvocation,
    command_overrides: ConfigOverrides,
) -> Result<AppContext, QtflowError> {
    let env = collect_env();
    let start = global.project.clone().unwrap_or(
        std::env::current_dir().map_err(|err| QtflowError::ConfigOrArg(err.to_string()))?,
    );
    let initial_project = discover_root(&start)?;
    let (project, config_path, raw) =
        resolve_project_and_config(global, &env, &start, initial_project)?;

    let mut overrides = command_overrides;
    overrides.profile = global.profile.clone();
    overrides.config_path = config_path.clone();

    let config = resolve(raw, &env, &overrides, &project.root);

    Ok(AppContext {
        project,
        config,
        env,
    })
}

fn resolve_project_and_config(
    global: &GlobalInvocation,
    env: &BTreeMap<String, String>,
    start: &Path,
    initial_project: ProjectContext,
) -> Result<(ProjectContext, Option<PathBuf>, Option<RawConfig>), QtflowError> {
    let initial_config_path = resolve_config_path(global, env, &initial_project)?
        .or_else(|| fallback_ancestor_config_path(global, env, &initial_project));
    let initial_raw = match &initial_config_path {
        Some(path) => Some(load_raw_config(path)?),
        None => None,
    };
    let provisional = resolve(
        initial_raw.clone(),
        env,
        &ConfigOverrides {
            config_path: initial_config_path.clone(),
            profile: global.profile.clone(),
            ..ConfigOverrides::default()
        },
        &initial_project.root,
    );
    let preference = discovery_preference(provisional.build_system);

    if preference == BuildSystemPreference::Auto {
        return Ok((initial_project, initial_config_path, initial_raw));
    }

    let project = discover_root_with_preference(start, preference)?;
    if !has_explicit_config(global, env) && initial_config_path.is_some() {
        return Ok((project, initial_config_path, initial_raw));
    }

    let config_path = resolve_config_path(global, env, &project)?;
    let raw = match &config_path {
        Some(path) => Some(load_raw_config(path)?),
        None => None,
    };

    Ok((project, config_path, raw))
}

fn has_explicit_config(global: &GlobalInvocation, env: &BTreeMap<String, String>) -> bool {
    global.config.is_some() || env.contains_key("QTFLOW_CONFIG")
}

fn fallback_ancestor_config_path(
    global: &GlobalInvocation,
    env: &BTreeMap<String, String>,
    initial_project: &ProjectContext,
) -> Option<PathBuf> {
    if has_explicit_config(global, env) {
        return None;
    }

    let mut current = initial_project.root.clone();
    loop {
        if !current.pop() {
            return None;
        }
        if let Some(config_path) = locate_project_config(&current) {
            return Some(config_path);
        }
    }
}

fn discovery_preference(build_system: BuildSystem) -> BuildSystemPreference {
    match build_system {
        BuildSystem::Auto => BuildSystemPreference::Auto,
        BuildSystem::Cmake => BuildSystemPreference::Cmake,
        BuildSystem::Qmake => BuildSystemPreference::Qmake,
    }
}

fn resolve_config_path(
    global: &GlobalInvocation,
    env: &BTreeMap<String, String>,
    project: &ProjectContext,
) -> Result<Option<PathBuf>, QtflowError> {
    let explicit = global
        .config
        .clone()
        .or_else(|| env.get("QTFLOW_CONFIG").map(PathBuf::from));

    if let Some(path) = explicit {
        if path.is_file() {
            return Ok(Some(path));
        }
        return Err(QtflowError::ConfigNotFound(path));
    }

    Ok(project.config_file.clone())
}

fn load_raw_config(path: &Path) -> Result<RawConfig, QtflowError> {
    let content = std::fs::read_to_string(path).map_err(|source| QtflowError::ConfigRead {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&content).map_err(|source| QtflowError::ConfigParse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })
}

fn collect_env() -> BTreeMap<String, String> {
    std::env::vars().collect()
}

trait ProvidesConfigOverrides {
    fn config_overrides(&self) -> ConfigOverrides;
}

impl ProvidesConfigOverrides for ConfigureInvocation {
    fn config_overrides(&self) -> ConfigOverrides {
        ConfigOverrides {
            vsdevcmd: self.vsdevcmd.clone(),
            ..ConfigOverrides::default()
        }
    }
}

impl ProvidesConfigOverrides for BuildInvocation {
    fn config_overrides(&self) -> ConfigOverrides {
        ConfigOverrides {
            vsdevcmd: self.vsdevcmd.clone(),
            ..ConfigOverrides::default()
        }
    }
}

impl ProvidesConfigOverrides for TestInvocation {
    fn config_overrides(&self) -> ConfigOverrides {
        ConfigOverrides {
            vsdevcmd: self.vsdevcmd.clone(),
            ..ConfigOverrides::default()
        }
    }
}

impl ProvidesConfigOverrides for CheckInvocation {
    fn config_overrides(&self) -> ConfigOverrides {
        ConfigOverrides {
            vsdevcmd: self.vsdevcmd.clone(),
            ..ConfigOverrides::default()
        }
    }
}

fn plan_overrides(args: &PlanInvocation) -> ConfigOverrides {
    match args {
        PlanInvocation::Configure(args) => args.config_overrides(),
        PlanInvocation::Build(args) => args.config_overrides(),
        PlanInvocation::Test(args) => args.config_overrides(),
        PlanInvocation::Check(args) => args.config_overrides(),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorReport {
    #[serde(serialize_with = "serialize_path")]
    project_root: PathBuf,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_path"
    )]
    config_file: Option<PathBuf>,
    config_source: String,
    profiles: Vec<String>,
    profile: String,
    selected_profile: DoctorProfile,
    build_system: String,
    #[serde(serialize_with = "serialize_path")]
    project_file: PathBuf,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_path"
    )]
    cmake_presets_file: Option<PathBuf>,
    cmake: DoctorTool,
    ctest: DoctorTool,
    qmake: DoctorQmake,
    cmake_presets: Vec<String>,
    discovered_build_dirs: Vec<DoctorDiscoveredBuildDir>,
    build_dir_warnings: Vec<DoctorBuildDirWarning>,
    msvc_bootstrap: DoctorMsvcBootstrap,
    #[serde(skip_serializing_if = "Option::is_none")]
    qt_hints: Option<qt::QtHints>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
    no_probe: bool,
    show_known_msvc: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    known_msvc_candidates: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct KnownMsvcReport {
    show_known_msvc: bool,
    known_msvc_candidates: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorProfile {
    name: String,
    #[serde(serialize_with = "serialize_path")]
    build_dir: PathBuf,
    preset: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorTool {
    path: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorQmake {
    path: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    spec: String,
    make: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rejected_candidates: Vec<DoctorQmakeRejectedCandidate>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorQmakeRejectedCandidate {
    path: String,
    reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorDiscoveredBuildDir {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    build_type: Option<String>,
    generator: String,
    multi_config: bool,
    provenance: String,
    role: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorBuildDirWarning {
    role: String,
    chosen: String,
    alternates: Vec<String>,
    hint: String,
    #[serde(skip)]
    display_alternates: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorMsvcBootstrap {
    status: String,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_path"
    )]
    path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    searched_sources: Vec<String>,
}

impl DoctorReport {
    fn from_context(ctx: &AppContext, args: &DoctorInvocation, probe: &impl Probe) -> Self {
        let profiles = ctx.config.profiles.keys().cloned().collect::<Vec<_>>();
        let selected = ctx
            .config
            .profiles
            .get(&ctx.config.active_profile)
            .expect("active profile is inserted during resolution");
        let known_msvc_candidates = if args.show_known_msvc {
            known_msvc_candidate_strings(&ctx.env)
        } else {
            Vec::new()
        };
        let cmake = DoctorTool::from_probe(&ctx.config.tools.cmake, args.no_probe, || {
            cmake::version(probe, &ctx.config.tools.cmake)
        });
        let ctest = DoctorTool::from_probe(&ctx.config.tools.ctest, args.no_probe, || {
            ctest::version(probe, &ctx.config.tools.ctest)
        });
        let qmake = DoctorQmake::from_context(ctx, args.no_probe, probe);
        let (cmake_presets, mut warnings) = list_presets_for_doctor(ctx);
        if ctx.project.kind == ProjectKind::Cmake {
            if let Some(preset) = &selected.preset {
                if !cmake_presets.iter().any(|name| name == preset) {
                    warnings.push(format!(
                    "configured preset '{preset}' for profile '{}' was not found in CMakePresets.json",
                    ctx.config.active_profile
                ));
                }
            }
        }
        let (discovered_build_dirs, build_dir_warnings) = discovered_build_dirs_for_doctor(ctx);
        let msvc_bootstrap = DoctorMsvcBootstrap::from_context(ctx, args.no_probe);
        let qt_hints = {
            let hints = qt::hints(&ctx.config.qt);
            (!hints.is_empty()).then_some(hints)
        };

        Self {
            project_root: ctx.project.root.clone(),
            config_file: match &ctx.config.source {
                ConfigSource::File(path) => Some(path.clone()),
                ConfigSource::Inferred => None,
            },
            config_source: config_source_display(&ctx.config.source),
            profiles,
            profile: ctx.config.active_profile.clone(),
            selected_profile: DoctorProfile::from_profile(&ctx.config.active_profile, selected),
            build_system: project_kind_display(ctx.project.kind).to_string(),
            project_file: ctx.project.project_file.clone(),
            cmake_presets_file: ctx.project.presets_file.clone(),
            cmake,
            ctest,
            qmake,
            cmake_presets,
            discovered_build_dirs,
            build_dir_warnings,
            msvc_bootstrap,
            qt_hints,
            warnings,
            no_probe: args.no_probe,
            show_known_msvc: args.show_known_msvc,
            known_msvc_candidates,
        }
    }
}

impl DoctorTool {
    fn from_probe(program: &str, no_probe: bool, version: impl FnOnce() -> Option<String>) -> Self {
        if no_probe {
            return Self {
                path: tool_path_display(program),
                status: "notProbed".to_string(),
                version: None,
            };
        }

        match version() {
            Some(version) => Self {
                path: tool_path_display(program),
                status: "found".to_string(),
                version: Some(version),
            },
            None => Self {
                path: tool_path_display(program),
                status: "notFound".to_string(),
                version: None,
            },
        }
    }
}

impl DoctorQmake {
    fn from_context(ctx: &AppContext, no_probe: bool, probe: &impl Probe) -> Self {
        let spec = qmake_spec_for_plan(ctx);
        let make = qmake_make_for_plan(ctx);
        let input = qmake::QmakeResolveInput::real(ctx.config.qmake.qmake.clone(), ctx.env.clone());
        let resolution = qmake::resolve_qmake(&input);
        let rejected_candidates = resolution
            .rejected
            .into_iter()
            .map(DoctorQmakeRejectedCandidate::from_rejected)
            .collect::<Vec<_>>();

        match resolution.path {
            Some(path) => {
                let display_path = path.to_string_lossy().replace('\\', "/");
                let version = if no_probe {
                    None
                } else {
                    qmake::version(probe, &display_path)
                };
                Self {
                    path: display_path,
                    status: if no_probe {
                        "notProbed".to_string()
                    } else if version.is_some() {
                        "found".to_string()
                    } else {
                        "notFound".to_string()
                    },
                    source: resolution.source.map(qmake_source_display),
                    version,
                    spec,
                    make,
                    rejected_candidates,
                }
            }
            None => Self {
                path: "qmake".to_string(),
                status: if no_probe {
                    "notProbed".to_string()
                } else {
                    "notFound".to_string()
                },
                source: None,
                version: None,
                spec,
                make,
                rejected_candidates,
            },
        }
    }
}

impl DoctorQmakeRejectedCandidate {
    fn from_rejected(candidate: qmake::QmakeRejectedCandidate) -> Self {
        Self {
            path: path_to_slash(&candidate.path),
            reason: qmake_rejection_reason_display(candidate.reason).to_string(),
        }
    }
}

impl DoctorMsvcBootstrap {
    fn from_context(ctx: &AppContext, no_probe: bool) -> Self {
        if !cfg!(windows) {
            return Self {
                status: "notApplicable".to_string(),
                path: None,
                source: None,
                searched_sources: Vec::new(),
            };
        }

        if !ctx.config.msvc.enabled {
            return Self {
                status: "disabled".to_string(),
                path: None,
                source: None,
                searched_sources: Vec::new(),
            };
        }

        let input = MsvcResolveInput::real(
            None,
            ctx.env.clone(),
            config_vsdevcmd_path(ctx),
            (!no_probe).then_some(&run_vswhere as &crate::core::detect::msvc::VsWhereRunner),
        );

        match resolve_vsdevcmd(&input) {
            VsDevCmdResolution::Found { path, source } => Self {
                status: "found".to_string(),
                path: Some(path),
                source: Some(vsdevcmd_source_display(source)),
                searched_sources: Vec::new(),
            },
            VsDevCmdResolution::NotFound { searched } => Self {
                status: "notFound".to_string(),
                path: None,
                source: None,
                searched_sources: searched.into_iter().map(vsdevcmd_source_display).collect(),
            },
        }
    }
}

impl DoctorProfile {
    fn from_profile(name: &str, profile: &Profile) -> Self {
        Self {
            name: name.to_string(),
            build_dir: profile.build_dir.clone(),
            preset: profile.preset.clone(),
        }
    }
}

fn print_doctor_text(report: &DoctorReport) {
    println!("Project root: {}", path_to_slash(&report.project_root));
    println!("Build system: {}", report.build_system);
    println!("Project file: {}", path_to_slash(&report.project_file));
    println!("Config file: {}", report.config_source);
    println!("Profiles: {}", report.profiles.join(", "));
    println!("Selected profile: {}", report.profile);
    println!(
        "Selected build dir: {}",
        path_to_slash(&report.selected_profile.build_dir)
    );
    println!(
        "Preset: {}",
        report
            .selected_profile
            .preset
            .as_deref()
            .unwrap_or("<none>")
    );
    println!(
        "CMake: {} ({})",
        report.cmake.path,
        report
            .cmake
            .version
            .as_deref()
            .unwrap_or(report.cmake.status.as_str())
    );
    println!(
        "CTest: {} ({})",
        report.ctest.path,
        report
            .ctest
            .version
            .as_deref()
            .unwrap_or(report.ctest.status.as_str())
    );
    println!(
        "qmake: {} ({}) spec={} make={}",
        report.qmake.path,
        report
            .qmake
            .version
            .as_deref()
            .unwrap_or(report.qmake.status.as_str()),
        report.qmake.spec,
        report.qmake.make
    );
    for candidate in &report.qmake.rejected_candidates {
        println!("qmake rejected: {} ({})", candidate.path, candidate.reason);
    }
    println!(
        "CMake presets: {}",
        if report.cmake_presets.is_empty() {
            "<none>".to_string()
        } else {
            report.cmake_presets.join(", ")
        }
    );
    println!("Discovered build directories:");
    if report.discovered_build_dirs.is_empty() {
        println!("  <none>");
    } else {
        for dir in &report.discovered_build_dirs {
            let role = dir.role.as_deref().unwrap_or("-");
            let build_type = dir.build_type.as_deref().unwrap_or("-");
            println!(
                "  {} build_type={} generator={} multi_config={} provenance={} role={}",
                dir.path, build_type, dir.generator, dir.multi_config, dir.provenance, role
            );
        }
    }
    match &report.msvc_bootstrap.path {
        Some(path) => println!(
            "MSVC bootstrap: {} via {}",
            path_to_slash(path),
            report
                .msvc_bootstrap
                .source
                .as_deref()
                .unwrap_or("<unknown>")
        ),
        None if report.msvc_bootstrap.searched_sources.is_empty() => {
            println!("MSVC bootstrap: {}", report.msvc_bootstrap.status)
        }
        None => println!(
            "MSVC bootstrap: {} (searched {})",
            report.msvc_bootstrap.status,
            report.msvc_bootstrap.searched_sources.join(", ")
        ),
    }
    if let Some(hints) = &report.qt_hints {
        if let Some(root) = &hints.root {
            println!("Qt root: {}", path_to_slash(root));
        }
        if let Some(bin_dir) = &hints.bin_dir {
            println!("Qt bin dir: {}", path_to_slash(bin_dir));
        }
    }
    for warning in &report.warnings {
        println!("warning: {warning}");
    }
    for warning in &report.build_dir_warnings {
        println!("{}", build_dir_warning_text(warning));
    }
    if report.show_known_msvc {
        print_known_msvc_text(&report.known_msvc_candidates);
    }
}

fn known_msvc_candidate_strings(env: &BTreeMap<String, String>) -> Vec<String> {
    known_vsdevcmd_candidates(env)
        .iter()
        .map(|path| path_to_slash(path))
        .collect()
}

fn print_known_msvc_text(candidates: &[String]) {
    println!("Known MSVC candidates:");
    for candidate in candidates {
        println!("  {candidate}");
    }
}

fn config_source_display(source: &ConfigSource) -> String {
    match source {
        ConfigSource::Inferred => "<inferred>".to_string(),
        ConfigSource::File(path) => path_to_slash(path),
    }
}

fn list_presets_for_doctor(ctx: &AppContext) -> (Vec<String>, Vec<String>) {
    match cmake::list_presets(&ctx.project.root) {
        Ok(presets) => (presets, Vec::new()),
        Err(err) => (
            Vec::new(),
            vec![format!("CMakePresets.json warning: {err}").replace('\\', "/")],
        ),
    }
}

fn discovered_build_dirs_for_doctor(
    ctx: &AppContext,
) -> (Vec<DoctorDiscoveredBuildDir>, Vec<DoctorBuildDirWarning>) {
    let dirs = discover_build_dirs(&ctx.project.root, &DiscoverOptions::default());
    let selection = classify(&dirs);
    let warnings = selection
        .warnings
        .iter()
        .map(DoctorBuildDirWarning::from_warning)
        .collect();
    let dirs = dirs
        .into_iter()
        .map(|dir| {
            let role = if selection.debug.as_ref() == Some(&dir) {
                Some("debug".to_string())
            } else if selection.release.as_ref() == Some(&dir) {
                Some("release".to_string())
            } else {
                None
            };
            DoctorDiscoveredBuildDir {
                path: path_to_slash(&dir.path),
                build_type: dir.build_type,
                generator: dir.generator,
                multi_config: dir.multi_config,
                provenance: provenance_json_name(dir.provenance).to_string(),
                role,
            }
        })
        .collect();
    (dirs, warnings)
}

impl DoctorBuildDirWarning {
    fn from_warning(warning: &AmbiguityWarning) -> Self {
        Self {
            role: warning.role.as_str().to_string(),
            chosen: path_to_slash(&warning.chosen.path),
            alternates: warning
                .alternates
                .iter()
                .map(|alternate| path_to_slash(&alternate.path))
                .collect(),
            hint: warning.hint.clone(),
            display_alternates: warning
                .alternates
                .iter()
                .map(warning_alternate_display)
                .collect(),
        }
    }
}

fn build_dir_warning_text(warning: &DoctorBuildDirWarning) -> String {
    let alternates = warning.display_alternates.join(", ");
    format!(
        "warning: multiple {} build dirs found: {} (chosen), {}; override with {}",
        warning.role, warning.chosen, alternates, warning.hint
    )
}

fn warning_alternate_display(dir: &crate::core::detect::builddir::DiscoveredBuildDir) -> String {
    let mut display = path_to_slash(&dir.path);
    if dir.provenance == Provenance::VisualStudio {
        display.push_str(" [VS]");
    }
    display
}

fn provenance_json_name(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::VisualStudio => "visualStudio",
        Provenance::Other => "other",
    }
}

fn project_kind_display(kind: ProjectKind) -> &'static str {
    match kind {
        ProjectKind::Cmake => "cmake",
        ProjectKind::Qmake => "qmake",
    }
}

fn tool_path_display(program: &str) -> String {
    program.replace('\\', "/")
}

fn qmake_source_display(source: qmake::QmakeSource) -> String {
    match source {
        qmake::QmakeSource::Config => "config",
        qmake::QmakeSource::EnvQtflow => "env:QTFLOW_QMAKE",
        qmake::QmakeSource::Path => "path",
    }
    .to_string()
}

fn qmake_rejection_reason_display(reason: qmake::QmakeRejectionReason) -> &'static str {
    match reason {
        qmake::QmakeRejectionReason::Conda => "conda",
    }
}

fn vsdevcmd_source_display(source: VsDevCmdSource) -> String {
    match source {
        VsDevCmdSource::Cli => "cli",
        VsDevCmdSource::EnvQtflow => "env:QTFLOW_VSDEVCMD_BAT",
        VsDevCmdSource::EnvCompat => "env:VSDEVCMD_BAT",
        VsDevCmdSource::Config => "config",
        VsDevCmdSource::VsInstallDir => "env:VSINSTALLDIR",
        VsDevCmdSource::VsWhere => "vswhere",
        VsDevCmdSource::KnownPath => "knownPath",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::detect::test_support::FakeProbe;
    use crate::core::plan::CommandStep;

    fn empty_plan() -> CommandPlan {
        CommandPlan {
            project_root: PathBuf::from("/repo"),
            profile: "debug".to_string(),
            steps: vec![
                CommandStep {
                    label: "build".to_string(),
                    cwd: PathBuf::from("/repo"),
                    program: "cmake".to_string(),
                    args: vec!["--build".to_string(), "/repo/build".to_string()],
                    env: BTreeMap::new(),
                    bootstrap: None,
                },
                CommandStep {
                    label: "test".to_string(),
                    cwd: PathBuf::from("/repo"),
                    program: "ctest".to_string(),
                    args: vec!["--test-dir".to_string(), "/repo/build".to_string()],
                    env: BTreeMap::new(),
                    bootstrap: None,
                },
            ],
        }
    }

    #[test]
    fn attach_execution_bootstrap_adds_msvc_to_all_steps() {
        let bootstrap = EnvironmentBootstrap::Msvc {
            vsdevcmd: PathBuf::from("C:/VS/Common7/Tools/VsDevCmd.bat"),
            arch: "x64".to_string(),
            host_arch: None,
        };

        let plan = attach_execution_bootstrap(empty_plan(), Some(bootstrap.clone()));

        assert_eq!(plan.steps[0].bootstrap, Some(bootstrap.clone()));
        assert_eq!(plan.steps[1].bootstrap, Some(bootstrap));
    }

    #[test]
    fn attach_execution_bootstrap_leaves_steps_unwrapped_when_none() {
        let plan = attach_execution_bootstrap(empty_plan(), None);

        assert!(plan.steps.iter().all(|step| step.bootstrap.is_none()));
    }

    #[test]
    fn doctor_report_uses_fake_probe_versions_and_presets() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("CMakeLists.txt"), "").expect("CMakeLists");
        std::fs::write(
            temp.path().join("CMakePresets.json"),
            r#"{"version": 6, "configurePresets": [{"name": "Qt-Debug"}]}"#,
        )
        .expect("presets");
        let project = ProjectContext {
            root: temp.path().to_path_buf(),
            kind: ProjectKind::Cmake,
            project_file: temp.path().join("CMakeLists.txt"),
            cmake_lists: Some(temp.path().join("CMakeLists.txt")),
            presets_file: Some(temp.path().join("CMakePresets.json")),
            config_file: None,
        };
        let config = resolve(
            None,
            &BTreeMap::new(),
            &ConfigOverrides::default(),
            temp.path(),
        );
        let ctx = AppContext {
            project,
            config,
            env: BTreeMap::new(),
        };
        let probe = FakeProbe::new([
            ("cmake", Some("cmake version 3.30.1".to_string())),
            ("ctest", Some("ctest version 3.30.1".to_string())),
        ]);

        let report = DoctorReport::from_context(&ctx, &DoctorInvocation::default(), &probe);

        assert_eq!(
            report.cmake.version.as_deref(),
            Some("cmake version 3.30.1")
        );
        assert_eq!(
            report.ctest.version.as_deref(),
            Some("ctest version 3.30.1")
        );
        assert_eq!(report.cmake_presets, vec!["Qt-Debug".to_string()]);
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn doctor_report_handles_missing_probe_and_preset_mismatch_warning() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("CMakeLists.txt"), "").expect("CMakeLists");
        std::fs::write(
            temp.path().join("CMakePresets.json"),
            r#"{"version": 6, "configurePresets": [{"name": "Other"}]}"#,
        )
        .expect("presets");
        let project = ProjectContext {
            root: temp.path().to_path_buf(),
            kind: ProjectKind::Cmake,
            project_file: temp.path().join("CMakeLists.txt"),
            cmake_lists: Some(temp.path().join("CMakeLists.txt")),
            presets_file: Some(temp.path().join("CMakePresets.json")),
            config_file: None,
        };
        let config = resolve(
            None,
            &BTreeMap::new(),
            &ConfigOverrides::default(),
            temp.path(),
        );
        let ctx = AppContext {
            project,
            config,
            env: BTreeMap::new(),
        };
        let probe = FakeProbe::new([("cmake", None), ("ctest", None)]);

        let report = DoctorReport::from_context(&ctx, &DoctorInvocation::default(), &probe);

        assert_eq!(report.cmake.status, "notFound");
        assert_eq!(report.ctest.status, "notFound");
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("was not found in CMakePresets.json")));
    }

    #[test]
    fn diagnostic_failure_report_maps_cmake_spawn_to_exit_three() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("CMakeLists.txt"), "").expect("CMakeLists");
        let project = ProjectContext {
            root: temp.path().to_path_buf(),
            kind: ProjectKind::Cmake,
            project_file: temp.path().join("CMakeLists.txt"),
            cmake_lists: Some(temp.path().join("CMakeLists.txt")),
            presets_file: None,
            config_file: None,
        };
        let config = resolve(
            None,
            &BTreeMap::new(),
            &ConfigOverrides::default(),
            temp.path(),
        );
        let ctx = AppContext {
            project,
            config,
            env: BTreeMap::new(),
        };

        let err = report_failed_step(
            &GlobalInvocation {
                quiet: true,
                ..GlobalInvocation::default()
            },
            &ctx,
            "build",
            "build",
            1,
            "No such file or directory\n",
            false,
        )
        .expect_err("reported failure");

        assert_eq!(err.exit_code(), 3);
    }
}
