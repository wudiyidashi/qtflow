use crate::core::path::path_to_slash;
use crate::core::plan::CommandPlan;
use crate::core::planners::{step, DeployBuildConfig, PlanCommand, PlanContext};
use crate::error::QtflowError;

pub fn plan(ctx: &PlanContext) -> Result<CommandPlan, QtflowError> {
    let PlanCommand::Deploy(inputs) = &ctx.command else {
        return Err(QtflowError::ConfigOrArg(
            "deploy planner received non-deploy inputs".to_string(),
        ));
    };

    let mut args = vec![
        path_to_slash(&inputs.exe),
        match inputs.config {
            DeployBuildConfig::Release => "--release".to_string(),
            DeployBuildConfig::Debug => "--debug".to_string(),
        },
    ];

    if let Some(qmldir) = &inputs.qmldir {
        args.extend(["--qmldir".to_string(), path_to_slash(qmldir)]);
    }
    if let Some(dir) = &inputs.dir {
        args.extend(["--dir".to_string(), path_to_slash(dir)]);
    }
    args.extend(inputs.deploy_args.clone());

    Ok(CommandPlan {
        project_root: ctx.project_root.clone(),
        profile: ctx.profile.clone(),
        notes: inputs.notes.clone(),
        steps: vec![step(ctx, "deploy", path_to_slash(&inputs.tool), args)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plan::EnvironmentBootstrap;
    use crate::core::planners::test_support::context;
    use crate::core::planners::{DeployPlanInputs, PlanCommand};
    use std::path::PathBuf;

    #[test]
    fn deploy_plan_uses_tool_exe_config_and_passthrough_args() {
        let ctx = context(PlanCommand::Deploy(DeployPlanInputs {
            tool: PathBuf::from("C:/Qt/bin/windeployqt.exe"),
            exe: PathBuf::from("C:/repo/out/build/debug/bin/app.exe"),
            config: DeployBuildConfig::Debug,
            qmldir: Some(PathBuf::from("qml")),
            dir: Some(PathBuf::from("C:/repo/package")),
            deploy_args: vec!["--compiler-runtime".to_string()],
            notes: Vec::new(),
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps[0].label, "deploy");
        assert_eq!(plan.steps[0].program, "C:/Qt/bin/windeployqt.exe");
        assert_eq!(
            plan.steps[0].args,
            vec![
                "C:/repo/out/build/debug/bin/app.exe",
                "--debug",
                "--qmldir",
                "qml",
                "--dir",
                "C:/repo/package",
                "--compiler-runtime"
            ]
        );
        assert_eq!(plan.steps[0].cwd, PathBuf::from("/repo"));
    }

    #[test]
    fn deploy_plan_uses_release_flag() {
        let ctx = context(PlanCommand::Deploy(DeployPlanInputs {
            config: DeployBuildConfig::Release,
            ..DeployPlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert!(plan.steps[0].args.iter().any(|arg| arg == "--release"));
        assert!(!plan.steps[0].args.iter().any(|arg| arg == "--debug"));
    }

    #[test]
    fn deploy_plan_carries_path_prepend_and_windows_bootstrap() {
        let mut ctx = context(PlanCommand::Deploy(DeployPlanInputs::default()));
        ctx.active_profile.path_prepend = vec!["C:/Qt/bin".to_string()];
        ctx.msvc.is_windows = true;
        ctx.msvc.enabled = true;
        ctx.msvc.vsdevcmd = Some(PathBuf::from("C:/VsDevCmd.bat"));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps[0].path_prepend, vec!["C:/Qt/bin"]);
        assert_eq!(
            plan.steps[0].bootstrap,
            Some(EnvironmentBootstrap::Msvc {
                vsdevcmd: PathBuf::from("C:/VsDevCmd.bat"),
                arch: "x64".to_string(),
                host_arch: None
            })
        );
    }

    #[test]
    fn deploy_plan_omits_bootstrap_for_non_windows_context() {
        let mut ctx = context(PlanCommand::Deploy(DeployPlanInputs::default()));
        ctx.msvc.is_windows = false;
        ctx.msvc.vsdevcmd = Some(PathBuf::from("C:/VsDevCmd.bat"));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps[0].bootstrap, None);
    }
}
