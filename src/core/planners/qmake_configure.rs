use crate::core::path::path_to_slash;
use crate::core::plan::CommandPlan;
use crate::core::planners::{step, PlanCommand, PlanContext, QmakeBuildConfig};
use crate::error::QtflowError;

pub fn plan(ctx: &PlanContext) -> Result<CommandPlan, QtflowError> {
    let PlanCommand::Configure(_) = &ctx.command else {
        return Err(QtflowError::ConfigOrArg(
            "qmake configure planner received non-configure inputs".to_string(),
        ));
    };

    let build_dir = &ctx.active_profile.build_dir;
    let makefile = build_dir.join("Makefile");
    let destdir = build_dir.join("bin");
    let mut args = vec![
        "-o".to_string(),
        path_to_slash(&makefile),
        path_to_slash(&ctx.qmake.pro_file),
        "-spec".to_string(),
        ctx.qmake.spec.clone(),
        format!("CONFIG+={}", qmake_config_arg(ctx.qmake.config)),
    ];
    args.extend(ctx.qmake.config_args.clone());
    args.extend([
        "-after".to_string(),
        format!("DESTDIR={}", path_to_slash(&destdir)),
    ]);

    Ok(CommandPlan {
        project_root: ctx.project_root.clone(),
        profile: ctx.profile.clone(),
        steps: vec![step(ctx, "configure", ctx.qmake.qmake.clone(), args)],
    })
}

fn qmake_config_arg(config: QmakeBuildConfig) -> &'static str {
    match config {
        QmakeBuildConfig::Debug => "debug",
        QmakeBuildConfig::Release => "release",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::planners::test_support::context;
    use crate::core::planners::{ConfigurePlanInputs, PlanCommand};

    #[test]
    fn qmake_configure_plan_uses_expected_args_for_debug() {
        let mut ctx = context(PlanCommand::Configure(ConfigurePlanInputs::default()));
        ctx.qmake.config_args = vec!["-recursive".to_string()];

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].cwd, ctx.project_root);
        assert_eq!(plan.steps[0].program, "qmake");
        assert_eq!(
            plan.steps[0].args,
            vec![
                "-o",
                "/repo/out/build/debug/Makefile",
                "/repo/app.pro",
                "-spec",
                "linux-g++",
                "CONFIG+=debug",
                "-recursive",
                "-after",
                "DESTDIR=/repo/out/build/debug/bin"
            ]
        );
    }

    #[test]
    fn qmake_configure_plan_uses_release_config() {
        let mut ctx = context(PlanCommand::Configure(ConfigurePlanInputs::default()));
        ctx.qmake.config = QmakeBuildConfig::Release;

        let plan = plan(&ctx).expect("plan");

        assert!(plan.steps[0]
            .args
            .iter()
            .any(|arg| arg == "CONFIG+=release"));
    }
}
