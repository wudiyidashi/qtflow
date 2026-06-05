use crate::core::plan::CommandPlan;
use crate::core::planners::build::effective_build_dir;
use crate::core::planners::{build_step, test_step, PlanCommand, PlanContext};
use crate::error::QtflowError;

pub fn plan(ctx: &PlanContext) -> Result<CommandPlan, QtflowError> {
    let PlanCommand::Check(inputs) = &ctx.command else {
        return Err(QtflowError::ConfigOrArg(
            "check planner received non-check inputs".to_string(),
        ));
    };

    let build_dir = effective_build_dir(ctx, inputs.build_dir.as_deref());
    let regex = inputs.test_regex.as_deref().unwrap_or(&inputs.target);

    Ok(CommandPlan {
        project_root: ctx.project_root.clone(),
        profile: ctx.profile.clone(),
        steps: vec![
            build_step(
                ctx,
                &build_dir,
                Some(&inputs.target),
                inputs.parallel,
                inputs.config_name.as_deref(),
            ),
            test_step(
                ctx,
                &build_dir,
                Some(regex),
                inputs.config_name.as_deref(),
                true,
                &inputs.ctest_arg,
            ),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::planners::test_support::context;
    use crate::core::planners::{CheckPlanInputs, PlanCommand};

    #[test]
    fn check_plan_builds_target_then_runs_matching_ctest() {
        let ctx = context(PlanCommand::Check(CheckPlanInputs {
            target: "foo".to_string(),
            ..CheckPlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].program, "cmake");
        assert_eq!(
            plan.steps[0].args,
            vec!["--build", "/repo/out/build/debug", "--target", "foo"]
        );
        assert_eq!(plan.steps[1].program, "ctest");
        assert_eq!(
            plan.steps[1].args,
            vec![
                "--test-dir",
                "/repo/out/build/debug",
                "-R",
                "foo",
                "--output-on-failure"
            ]
        );
    }

    #[test]
    fn check_plan_applies_config_name_to_build_and_ctest_steps() {
        let ctx = context(PlanCommand::Check(CheckPlanInputs {
            target: "foo".to_string(),
            config_name: Some("Debug".to_string()),
            ..CheckPlanInputs::default()
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
                "foo"
            ]
        );
        assert_eq!(
            plan.steps[1].args,
            vec![
                "--test-dir",
                "/repo/out/build/debug",
                "-C",
                "Debug",
                "-R",
                "foo",
                "--output-on-failure"
            ]
        );
    }

    #[test]
    fn check_test_regex_override_and_ctest_args_append() {
        let ctx = context(PlanCommand::Check(CheckPlanInputs {
            target: "foo".to_string(),
            test_regex: Some("smoke".to_string()),
            ctest_arg: vec![
                "--schedule-random".to_string(),
                "--timeout".to_string(),
                "30".to_string(),
            ],
            ..CheckPlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(
            plan.steps[1].args,
            vec![
                "--test-dir",
                "/repo/out/build/debug",
                "-R",
                "smoke",
                "--output-on-failure",
                "--schedule-random",
                "--timeout",
                "30"
            ]
        );
    }
}
