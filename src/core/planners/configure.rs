use crate::core::path::path_to_slash;
use crate::core::plan::CommandPlan;
use crate::core::planners::{step, PlanCommand, PlanContext};
use crate::error::QtflowError;

pub fn plan(ctx: &PlanContext) -> Result<CommandPlan, QtflowError> {
    let PlanCommand::Configure(inputs) = &ctx.command else {
        return Err(QtflowError::ConfigOrArg(
            "configure planner received non-configure inputs".to_string(),
        ));
    };

    let mut args = if let Some(preset) = inputs
        .preset
        .as_ref()
        .or(ctx.active_profile.preset.as_ref())
    {
        vec!["--preset".to_string(), preset.clone()]
    } else if let Some(generator) = inputs
        .generator
        .as_ref()
        .or(ctx.active_profile.generator.as_ref())
    {
        vec![
            "-S".to_string(),
            path_to_slash(&ctx.project_root),
            "-B".to_string(),
            path_to_slash(&ctx.active_profile.build_dir),
            "-G".to_string(),
            generator.clone(),
        ]
    } else {
        return Err(QtflowError::ConfigOrArg(format!(
            "no CMake preset or generator configured for profile '{}'; set profiles.{}.preset or profiles.{}.generator, or pass --preset/--generator",
            ctx.profile, ctx.profile, ctx.profile
        )));
    };
    args.extend(ctx.active_profile.configure_args.clone());
    if inputs.fresh {
        args.push("--fresh".to_string());
    }

    Ok(CommandPlan {
        project_root: ctx.project_root.clone(),
        profile: ctx.profile.clone(),
        steps: vec![step(ctx, "configure", ctx.tools.cmake.clone(), args)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::path::path_to_slash;
    use crate::core::planners::test_support::context;
    use crate::core::planners::{ConfigurePlanInputs, PlanCommand};

    #[test]
    fn configure_plan_uses_preset_and_profile_args() {
        let mut ctx = context(PlanCommand::Configure(ConfigurePlanInputs::default()));
        ctx.active_profile.configure_args = vec!["-DOPT=ON".to_string()];

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].program, "cmake");
        assert_eq!(plan.steps[0].args, vec!["--preset", "Qt-Debug", "-DOPT=ON"]);
    }

    #[test]
    fn configure_plan_uses_preset_and_fresh() {
        let ctx = context(PlanCommand::Configure(ConfigurePlanInputs {
            fresh: true,
            ..ConfigurePlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps[0].args, vec!["--preset", "Qt-Debug", "--fresh"]);
    }

    #[test]
    fn configure_cli_preset_overrides_profile_preset() {
        let ctx = context(PlanCommand::Configure(ConfigurePlanInputs {
            preset: Some("Qt-Custom".to_string()),
            ..ConfigurePlanInputs::default()
        }));

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps[0].args, vec!["--preset", "Qt-Custom"]);
    }

    #[test]
    fn configure_uses_profile_generator_without_preset() {
        let mut ctx = context(PlanCommand::Configure(ConfigurePlanInputs::default()));
        ctx.active_profile.preset = None;
        ctx.active_profile.generator = Some("Ninja".to_string());

        let plan = plan(&ctx).expect("plan");

        assert_eq!(
            plan.steps[0].args,
            vec!["-S", "/repo", "-B", "/repo/out/build/debug", "-G", "Ninja"]
        );
    }

    #[test]
    fn configure_cli_generator_overrides_profile_generator() {
        let mut ctx = context(PlanCommand::Configure(ConfigurePlanInputs {
            generator: Some("Unix Makefiles".to_string()),
            ..ConfigurePlanInputs::default()
        }));
        ctx.active_profile.preset = None;
        ctx.active_profile.generator = Some("Ninja".to_string());

        let plan = plan(&ctx).expect("plan");

        assert_eq!(
            plan.steps[0].args,
            vec![
                "-S",
                "/repo",
                "-B",
                "/repo/out/build/debug",
                "-G",
                "Unix Makefiles"
            ]
        );
    }

    #[test]
    fn configure_cli_generator_works_without_profile_generator() {
        let mut ctx = context(PlanCommand::Configure(ConfigurePlanInputs {
            generator: Some("Ninja".to_string()),
            ..ConfigurePlanInputs::default()
        }));
        ctx.active_profile.preset = None;
        ctx.active_profile.generator = None;

        let plan = plan(&ctx).expect("plan");

        assert_eq!(
            plan.steps[0].args,
            vec!["-S", "/repo", "-B", "/repo/out/build/debug", "-G", "Ninja"]
        );
    }

    #[test]
    fn configure_fresh_appends_to_generator_form() {
        let mut ctx = context(PlanCommand::Configure(ConfigurePlanInputs {
            fresh: true,
            ..ConfigurePlanInputs::default()
        }));
        ctx.active_profile.preset = None;
        ctx.active_profile.generator = Some("Ninja".to_string());

        let plan = plan(&ctx).expect("plan");

        assert_eq!(
            plan.steps[0].args,
            vec![
                "-S",
                "/repo",
                "-B",
                "/repo/out/build/debug",
                "-G",
                "Ninja",
                "--fresh"
            ]
        );
    }

    #[test]
    fn configure_cli_preset_wins_over_cli_generator() {
        let mut ctx = context(PlanCommand::Configure(ConfigurePlanInputs {
            preset: Some("Qt-Custom".to_string()),
            generator: Some("Ninja".to_string()),
            ..ConfigurePlanInputs::default()
        }));
        ctx.active_profile.preset = None;

        let plan = plan(&ctx).expect("plan");

        assert_eq!(plan.steps[0].args, vec!["--preset", "Qt-Custom"]);
    }

    #[test]
    fn configure_errors_without_preset_or_generator() {
        let mut ctx = context(PlanCommand::Configure(ConfigurePlanInputs::default()));
        ctx.active_profile.preset = None;
        ctx.active_profile.generator = None;

        let err = plan(&ctx).expect_err("missing preset or generator");

        assert_eq!(err.exit_code(), 2);
        assert_eq!(
            err.to_string(),
            "no CMake preset or generator configured for profile 'debug'; set profiles.debug.preset or profiles.debug.generator, or pass --preset/--generator"
        );
    }

    #[test]
    fn configure_serializes_slash_paths() {
        let mut ctx = context(PlanCommand::Configure(ConfigurePlanInputs::default()));
        ctx.project_root = "D:\\repo".into();

        let plan = plan(&ctx).expect("plan");
        let json = serde_json::to_string(&plan).expect("json");

        assert!(json.contains("D:/repo"));
        assert!(!json.contains(r"D:\\repo"));
        assert_eq!(path_to_slash(&ctx.project_root), "D:/repo");
    }
}
