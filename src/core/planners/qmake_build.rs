use std::path::{Path, PathBuf};

use crate::core::path::path_to_slash;
use crate::core::plan::{CommandPlan, CommandStep};
use crate::core::planners::{step_with_cwd, PlanCommand, PlanContext};
use crate::error::QtflowError;

pub fn plan(ctx: &PlanContext) -> Result<CommandPlan, QtflowError> {
    let PlanCommand::Build(inputs) = &ctx.command else {
        return Err(QtflowError::ConfigOrArg(
            "qmake build planner received non-build inputs".to_string(),
        ));
    };

    let build_dir = effective_build_dir(ctx, inputs.build_dir.as_deref());
    let target = (!inputs.all).then_some(inputs.target.as_deref()).flatten();

    Ok(CommandPlan {
        project_root: ctx.project_root.clone(),
        profile: ctx.profile.clone(),
        steps: vec![qmake_build_step(ctx, &build_dir, target)],
    })
}

fn qmake_build_step(ctx: &PlanContext, build_dir: &Path, target: Option<&str>) -> CommandStep {
    let make_name = make_tool_name(&ctx.qmake.make);
    let mut args = if make_name == "nmake" || make_name == "jom" {
        Vec::new()
    } else {
        vec!["-C".to_string(), path_to_slash(build_dir)]
    };
    if let Some(target) = target {
        args.push(target.to_string());
    }
    args.extend(ctx.active_profile.build_args.clone());

    let cwd = if make_name == "nmake" || make_name == "jom" {
        build_dir.to_path_buf()
    } else {
        ctx.project_root.clone()
    };

    step_with_cwd(ctx, "build", cwd, ctx.qmake.make.clone(), args)
}

fn effective_build_dir(ctx: &PlanContext, override_dir: Option<&Path>) -> PathBuf {
    match override_dir {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => ctx.project_root.join(path),
        None => ctx.active_profile.build_dir.clone(),
    }
}

fn make_tool_name(make: &str) -> String {
    Path::new(&make.replace('\\', "/"))
        .file_stem()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| make.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plan::EnvironmentBootstrap;
    use crate::core::planners::test_support::context;
    use crate::core::planners::{BuildPlanInputs, PlanCommand};

    #[test]
    fn nmake_build_uses_build_dir_cwd_and_no_dash_c() {
        let mut ctx = context(PlanCommand::Build(BuildPlanInputs {
            target: Some("app".to_string()),
            ..BuildPlanInputs::default()
        }));
        ctx.qmake.make = "nmake".to_string();

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps[0].program, "nmake");
        assert_eq!(plan.steps[0].cwd, PathBuf::from("/repo/out/build/debug"));
        assert_eq!(plan.steps[0].args, vec!["app"]);
    }

    #[test]
    fn make_build_uses_dash_c_build_dir() {
        let ctx = context(PlanCommand::Build(BuildPlanInputs {
            target: Some("app".to_string()),
            ..BuildPlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps[0].program, "make");
        assert_eq!(plan.steps[0].cwd, PathBuf::from("/repo"));
        assert_eq!(
            plan.steps[0].args,
            vec!["-C", "/repo/out/build/debug", "app"]
        );
    }

    #[test]
    fn windows_context_with_vsdevcmd_carries_msvc_bootstrap() {
        let mut ctx = context(PlanCommand::Build(BuildPlanInputs::default()));
        ctx.msvc.is_windows = true;
        ctx.msvc.enabled = true;
        ctx.msvc.vsdevcmd = Some(PathBuf::from("C:/VsDevCmd.bat"));
        ctx.qmake.make = "nmake".to_string();

        let plan = plan(&ctx).expect("plan");

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
    fn non_windows_context_has_no_bootstrap() {
        let mut ctx = context(PlanCommand::Build(BuildPlanInputs::default()));
        ctx.msvc.is_windows = false;
        ctx.msvc.vsdevcmd = Some(PathBuf::from("C:/VsDevCmd.bat"));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps[0].bootstrap, None);
    }
}
