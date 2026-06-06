use crate::core::plan::CommandPlan;
use crate::core::planners::build::effective_build_dir;
use crate::core::planners::{build_step, test_step, PlanCommand, PlanContext};
use crate::error::QtflowError;

pub fn plan(ctx: &PlanContext) -> Result<CommandPlan, QtflowError> {
    let PlanCommand::Test(inputs) = &ctx.command else {
        return Err(QtflowError::ConfigOrArg(
            "test planner received non-test inputs".to_string(),
        ));
    };

    let build_dir = effective_build_dir(ctx, inputs.build_dir.as_deref());
    let mut steps = Vec::new();

    if let Some(build_target) = &inputs.build_target {
        steps.push(build_step(
            ctx,
            &build_dir,
            Some(build_target),
            inputs.parallel,
            inputs.config_name.as_deref(),
        ));
    }

    steps.push(test_step(
        ctx,
        &build_dir,
        inputs.regex.as_deref(),
        inputs.config_name.as_deref(),
        !inputs.no_output_on_failure,
        &inputs.ctest_arg,
    ));

    Ok(CommandPlan {
        project_root: ctx.project_root.clone(),
        profile: ctx.profile.clone(),
        notes: Vec::new(),
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::planners::test_support::context;
    use crate::core::planners::{PlanCommand, TestPlanInputs};

    #[test]
    fn test_plan_builds_optional_target_then_runs_ctest() {
        let ctx = context(PlanCommand::Test(TestPlanInputs {
            regex: Some("route".to_string()),
            build_target: Some("route_test".to_string()),
            parallel: Some(4),
            ..TestPlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].program, "cmake");
        assert_eq!(
            plan.steps[0].args,
            vec![
                "--build",
                "/repo/out/build/debug",
                "--target",
                "route_test",
                "--parallel",
                "4"
            ]
        );
        assert_eq!(plan.steps[1].program, "ctest");
        assert_eq!(
            plan.steps[1].args,
            vec![
                "--test-dir",
                "/repo/out/build/debug",
                "-R",
                "route",
                "--output-on-failure"
            ]
        );
    }

    #[test]
    fn test_plan_applies_config_name_to_build_and_ctest_steps() {
        let ctx = context(PlanCommand::Test(TestPlanInputs {
            regex: Some("route".to_string()),
            build_target: Some("route_test".to_string()),
            config_name: Some("Release".to_string()),
            ..TestPlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(
            plan.steps[0].args,
            vec![
                "--build",
                "/repo/out/build/debug",
                "--config",
                "Release",
                "--target",
                "route_test"
            ]
        );
        assert_eq!(
            plan.steps[1].args,
            vec![
                "--test-dir",
                "/repo/out/build/debug",
                "-C",
                "Release",
                "-R",
                "route",
                "--output-on-failure"
            ]
        );
    }

    #[test]
    fn no_output_on_failure_omits_standard_flag_and_ctest_args_append() {
        let mut ctx = context(PlanCommand::Test(TestPlanInputs {
            regex: Some("route".to_string()),
            no_output_on_failure: true,
            ctest_arg: vec!["--label-exclude".to_string(), "slow".to_string()],
            ..TestPlanInputs::default()
        }));
        ctx.active_profile.ctest_args = vec!["--output-on-failure".to_string(), "-V".to_string()];

        let plan = plan(&ctx).expect("plan");

        assert_eq!(
            plan.steps[0].args,
            vec![
                "--test-dir",
                "/repo/out/build/debug",
                "-R",
                "route",
                "--label-exclude",
                "slow",
                "-V"
            ]
        );
    }
}
