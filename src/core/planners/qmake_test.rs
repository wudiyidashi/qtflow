use crate::core::plan::CommandPlan;
use crate::core::planners::qmake_build::{effective_build_dir, qmake_build_step, qmake_check_step};
use crate::core::planners::{PlanCommand, PlanContext};
use crate::error::QtflowError;

pub fn plan(ctx: &PlanContext) -> Result<CommandPlan, QtflowError> {
    let PlanCommand::Test(inputs) = &ctx.command else {
        return Err(QtflowError::ConfigOrArg(
            "qmake test planner received non-test inputs".to_string(),
        ));
    };

    let build_dir = effective_build_dir(ctx, inputs.build_dir.as_deref());
    let mut steps = Vec::new();

    if let Some(build_target) = &inputs.build_target {
        steps.push(qmake_build_step(ctx, &build_dir, Some(build_target)));
    }

    steps.push(qmake_check_step(ctx, &build_dir));

    Ok(CommandPlan {
        project_root: ctx.project_root.clone(),
        profile: ctx.profile.clone(),
        notes: qmake_regex_note(inputs.regex.as_deref())
            .into_iter()
            .collect(),
        steps,
    })
}

pub(super) fn qmake_regex_note(regex: Option<&str>) -> Option<String> {
    regex.map(|regex| {
        format!(
            "qmake runs all tests exposed by the generated Makefile check target; regex '{regex}' is ignored."
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plan::EnvironmentBootstrap;
    use crate::core::planners::test_support::context;
    use crate::core::planners::{PlanCommand, TestPlanInputs};
    use std::path::PathBuf;

    #[test]
    fn nmake_test_runs_check_from_build_dir_without_dash_c() {
        let mut ctx = context(PlanCommand::Test(TestPlanInputs::default()));
        ctx.qmake.make = "nmake".to_string();

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].label, "test");
        assert_eq!(plan.steps[0].program, "nmake");
        assert_eq!(plan.steps[0].cwd, PathBuf::from("/repo/out/build/debug"));
        assert_eq!(plan.steps[0].args, vec!["check"]);
    }

    #[test]
    fn make_test_runs_check_with_dash_c_build_dir() {
        let ctx = context(PlanCommand::Test(TestPlanInputs::default()));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps[0].program, "make");
        assert_eq!(plan.steps[0].cwd, PathBuf::from("/repo"));
        assert_eq!(
            plan.steps[0].args,
            vec!["-C", "/repo/out/build/debug", "check"]
        );
    }

    #[test]
    fn build_target_adds_build_step_before_check() {
        let ctx = context(PlanCommand::Test(TestPlanInputs {
            build_target: Some("app_test".to_string()),
            ..TestPlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].label, "build");
        assert_eq!(
            plan.steps[0].args,
            vec!["-C", "/repo/out/build/debug", "app_test"]
        );
        assert_eq!(plan.steps[1].label, "test");
        assert_eq!(
            plan.steps[1].args,
            vec!["-C", "/repo/out/build/debug", "check"]
        );
    }

    #[test]
    fn regex_is_ignored_with_plan_note() {
        let ctx = context(PlanCommand::Test(TestPlanInputs {
            regex: Some("smoke".to_string()),
            ..TestPlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(
            plan.notes,
            vec!["qmake runs all tests exposed by the generated Makefile check target; regex 'smoke' is ignored."]
        );
        assert_eq!(
            plan.steps[0].args,
            vec!["-C", "/repo/out/build/debug", "check"]
        );
    }

    #[test]
    fn check_step_carries_path_prepend_and_msvc_bootstrap() {
        let mut ctx = context(PlanCommand::Test(TestPlanInputs::default()));
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
}
