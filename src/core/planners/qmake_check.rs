use crate::core::plan::CommandPlan;
use crate::core::planners::qmake_build::{effective_build_dir, qmake_build_step, qmake_check_step};
use crate::core::planners::qmake_test::qmake_regex_note;
use crate::core::planners::{PlanCommand, PlanContext};
use crate::error::QtflowError;

pub fn plan(ctx: &PlanContext) -> Result<CommandPlan, QtflowError> {
    let PlanCommand::Check(inputs) = &ctx.command else {
        return Err(QtflowError::ConfigOrArg(
            "qmake check planner received non-check inputs".to_string(),
        ));
    };

    let build_dir = effective_build_dir(ctx, inputs.build_dir.as_deref());

    Ok(CommandPlan {
        project_root: ctx.project_root.clone(),
        profile: ctx.profile.clone(),
        notes: qmake_regex_note(inputs.test_regex.as_deref())
            .into_iter()
            .collect(),
        steps: vec![
            qmake_build_step(ctx, &build_dir, Some(&inputs.target)),
            qmake_check_step(ctx, &build_dir),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::planners::test_support::context;
    use crate::core::planners::{CheckPlanInputs, PlanCommand};
    use std::path::PathBuf;

    #[test]
    fn check_builds_target_then_runs_make_check() {
        let ctx = context(PlanCommand::Check(CheckPlanInputs {
            target: "foo".to_string(),
            ..CheckPlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].label, "build");
        assert_eq!(
            plan.steps[0].args,
            vec!["-C", "/repo/out/build/debug", "foo"]
        );
        assert_eq!(plan.steps[1].label, "test");
        assert_eq!(
            plan.steps[1].args,
            vec!["-C", "/repo/out/build/debug", "check"]
        );
    }

    #[test]
    fn check_with_nmake_uses_build_dir_cwd_for_both_steps() {
        let mut ctx = context(PlanCommand::Check(CheckPlanInputs {
            target: "foo".to_string(),
            ..CheckPlanInputs::default()
        }));
        ctx.qmake.make = "nmake".to_string();

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps[0].program, "nmake");
        assert_eq!(plan.steps[0].cwd, PathBuf::from("/repo/out/build/debug"));
        assert_eq!(plan.steps[0].args, vec!["foo"]);
        assert_eq!(plan.steps[1].program, "nmake");
        assert_eq!(plan.steps[1].cwd, PathBuf::from("/repo/out/build/debug"));
        assert_eq!(plan.steps[1].args, vec!["check"]);
    }

    #[test]
    fn explicit_test_regex_is_ignored_with_plan_note() {
        let ctx = context(PlanCommand::Check(CheckPlanInputs {
            target: "foo".to_string(),
            test_regex: Some("smoke".to_string()),
            ..CheckPlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(
            plan.notes,
            vec!["qmake runs all tests exposed by the generated Makefile check target; regex 'smoke' is ignored."]
        );
    }
}
