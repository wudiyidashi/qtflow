use std::path::{Path, PathBuf};

use crate::core::plan::CommandPlan;
use crate::core::planners::{build_step, PlanCommand, PlanContext};
use crate::error::QtflowError;

pub fn plan(ctx: &PlanContext) -> Result<CommandPlan, QtflowError> {
    let PlanCommand::Build(inputs) = &ctx.command else {
        return Err(QtflowError::ConfigOrArg(
            "build planner received non-build inputs".to_string(),
        ));
    };

    let build_dir = effective_build_dir(ctx, inputs.build_dir.as_deref());
    let target = (!inputs.all).then_some(inputs.target.as_deref()).flatten();

    Ok(CommandPlan {
        project_root: ctx.project_root.clone(),
        profile: ctx.profile.clone(),
        notes: Vec::new(),
        steps: vec![build_step(
            ctx,
            &build_dir,
            target,
            inputs.parallel,
            inputs.config_name.as_deref(),
        )],
    })
}

pub(super) fn effective_build_dir(ctx: &PlanContext, override_dir: Option<&Path>) -> PathBuf {
    match override_dir {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => ctx.project_root.join(path),
        None => ctx.active_profile.build_dir.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::planners::test_support::context;
    use crate::core::planners::{BuildPlanInputs, PlanCommand};

    #[test]
    fn build_plan_uses_target_parallel_and_profile_args() {
        let mut ctx = context(PlanCommand::Build(BuildPlanInputs {
            target: Some("app".to_string()),
            parallel: Some(8),
            ..BuildPlanInputs::default()
        }));
        ctx.active_profile.build_args = vec!["--verbose".to_string()];

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].program, "cmake");
        assert_eq!(
            plan.steps[0].args,
            vec![
                "--build",
                "/repo/out/build/debug",
                "--target",
                "app",
                "--parallel",
                "8",
                "--verbose"
            ]
        );
    }

    #[test]
    fn build_plan_inserts_config_name_after_build_dir() {
        let ctx = context(PlanCommand::Build(BuildPlanInputs {
            target: Some("app".to_string()),
            config_name: Some("Debug".to_string()),
            ..BuildPlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(
            plan.steps[0].args,
            vec![
                "--build",
                "/repo/out/build/debug",
                "--config",
                "Debug",
                "--target",
                "app"
            ]
        );
    }

    #[test]
    fn build_all_omits_target() {
        let ctx = context(PlanCommand::Build(BuildPlanInputs {
            target: Some("app".to_string()),
            all: true,
            ..BuildPlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps[0].args, vec!["--build", "/repo/out/build/debug"]);
    }

    #[test]
    fn build_dir_override_is_resolved_against_project_root() {
        let ctx = context(PlanCommand::Build(BuildPlanInputs {
            build_dir: Some(PathBuf::from("custom/build")),
            ..BuildPlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps[0].args, vec!["--build", "/repo/custom/build"]);
    }
}
