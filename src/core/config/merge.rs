use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::model::{
    BuildSystem, ConfigSource, DiagnosticsConfig, MsvcConfig, Profile, QmakeConfig, QtConfig,
    ResolvedConfig, TestPreset, Tools,
};
use super::raw::{RawBuildSystem, RawConfig, RawProfile};

pub const DEFAULT_PROFILE_NAME: &str = "debug";
pub const DEBUG_PROFILE_NAME: &str = "debug";
pub const RELEASE_PROFILE_NAME: &str = "release";
pub const DEBUG_PRESET_NAME: &str = "Qt-Debug";
pub const RELEASE_PRESET_NAME: &str = "Qt-Release";
pub const DEBUG_BUILD_DIR: &str = "out/build/debug";
pub const RELEASE_BUILD_DIR: &str = "out/build/release";
pub const DEFAULT_CTEST_ARG: &str = "--output-on-failure";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigOverrides {
    pub config_path: Option<PathBuf>,
    pub profile: Option<String>,
    pub cmake: Option<String>,
    pub ctest: Option<String>,
    pub qmake: Option<String>,
    pub vsdevcmd: Option<PathBuf>,
    pub msvc_enabled: Option<bool>,
}

pub fn resolve(
    raw: Option<RawConfig>,
    env: &BTreeMap<String, String>,
    cli_overrides: &ConfigOverrides,
    project_root: &Path,
) -> ResolvedConfig {
    let mut cfg = inferred_defaults(project_root);
    let had_raw = raw.is_some();

    if let Some(raw) = raw {
        cfg.warnings.extend(raw.unknown_key_warnings());
        apply_file(&mut cfg, raw, project_root);
    }

    apply_env(&mut cfg, env);
    apply_cli(&mut cfg, cli_overrides);

    if let Some(config_path) = &cli_overrides.config_path {
        cfg.source = ConfigSource::File(config_path.clone());
    } else if had_raw {
        if let Some(config_path) = env.get("QTFLOW_CONFIG") {
            cfg.source = ConfigSource::File(PathBuf::from(config_path));
        }
    }

    cfg.active_profile = cli_overrides
        .profile
        .clone()
        .or_else(|| env.get("QTFLOW_PROFILE").cloned())
        .unwrap_or_else(|| cfg.default_profile.clone());

    if !cfg.profiles.contains_key(&cfg.active_profile) {
        let build_dir = absolutize(
            project_root,
            Path::new("out").join("build").join(&cfg.active_profile),
        );
        cfg.profiles.insert(
            cfg.active_profile.clone(),
            Profile {
                preset: None,
                build_dir,
                generator: None,
                config_name: None,
                configure_args: Vec::new(),
                cache_variables: BTreeMap::new(),
                build_args: Vec::new(),
                ctest_args: vec![DEFAULT_CTEST_ARG.to_string()],
                path_prepend: Vec::new(),
                env: BTreeMap::new(),
            },
        );
    }

    cfg
}

fn inferred_defaults(project_root: &Path) -> ResolvedConfig {
    let mut profiles = BTreeMap::new();
    profiles.insert(
        DEBUG_PROFILE_NAME.to_string(),
        Profile {
            preset: Some(DEBUG_PRESET_NAME.to_string()),
            build_dir: absolutize(project_root, DEBUG_BUILD_DIR),
            generator: None,
            config_name: None,
            configure_args: Vec::new(),
            cache_variables: BTreeMap::new(),
            build_args: Vec::new(),
            ctest_args: vec![DEFAULT_CTEST_ARG.to_string()],
            path_prepend: Vec::new(),
            env: BTreeMap::new(),
        },
    );
    profiles.insert(
        RELEASE_PROFILE_NAME.to_string(),
        Profile {
            preset: Some(RELEASE_PRESET_NAME.to_string()),
            build_dir: absolutize(project_root, RELEASE_BUILD_DIR),
            generator: None,
            config_name: None,
            configure_args: Vec::new(),
            cache_variables: BTreeMap::new(),
            build_args: Vec::new(),
            ctest_args: vec![DEFAULT_CTEST_ARG.to_string()],
            path_prepend: Vec::new(),
            env: BTreeMap::new(),
        },
    );

    ResolvedConfig {
        default_profile: DEFAULT_PROFILE_NAME.to_string(),
        active_profile: DEFAULT_PROFILE_NAME.to_string(),
        build_system: BuildSystem::Auto,
        tools: Tools {
            cmake: "cmake".to_string(),
            ctest: "ctest".to_string(),
            ninja: None,
        },
        msvc: MsvcConfig {
            enabled: true,
            arch: "x64".to_string(),
            host_arch: None,
            vsdevcmd: None,
        },
        qt: QtConfig {
            root: None,
            bin_dir: None,
        },
        qmake: QmakeConfig {
            qmake: None,
            spec: None,
            make: None,
            pro_file: None,
            config_args: Vec::new(),
        },
        profiles,
        tests: BTreeMap::new(),
        diagnostics: DiagnosticsConfig {
            enabled: true,
            max_log_bytes: 200_000,
        },
        source: ConfigSource::Inferred,
        warnings: Vec::new(),
    }
}

fn apply_file(cfg: &mut ResolvedConfig, raw: RawConfig, project_root: &Path) {
    if let Some(default_profile) = raw.default_profile {
        cfg.default_profile = default_profile;
    }
    if let Some(build_system) = raw.build_system {
        cfg.build_system = match build_system {
            RawBuildSystem::Auto => BuildSystem::Auto,
            RawBuildSystem::Cmake => BuildSystem::Cmake,
            RawBuildSystem::Qmake => BuildSystem::Qmake,
        };
    }

    if let Some(tools) = raw.tools {
        if let Some(cmake) = tools.cmake {
            cfg.tools.cmake = cmake;
        }
        if let Some(ctest) = tools.ctest {
            cfg.tools.ctest = ctest;
        }
        if let Some(ninja) = tools.ninja {
            cfg.tools.ninja = empty_string_to_none(ninja);
        }
    }

    if let Some(msvc) = raw.msvc {
        if let Some(enabled) = msvc.enabled {
            cfg.msvc.enabled = enabled;
        }
        if let Some(arch) = msvc.arch {
            cfg.msvc.arch = arch;
        }
        if let Some(host_arch) = msvc.host_arch {
            cfg.msvc.host_arch = empty_string_to_none(host_arch);
        }
        if let Some(vsdevcmd) = msvc.vsdevcmd {
            cfg.msvc.vsdevcmd = empty_path_to_none(vsdevcmd);
        }
    }

    if let Some(qt) = raw.qt {
        if let Some(root) = qt.root {
            cfg.qt.root = empty_path_to_none(root);
        }
        if let Some(bin_dir) = qt.bin_dir {
            cfg.qt.bin_dir = empty_path_to_none(bin_dir);
        }
    }

    if let Some(qmake) = raw.qmake {
        if let Some(program) = qmake.qmake {
            cfg.qmake.qmake = empty_string_to_none(program);
        }
        if let Some(spec) = qmake.spec {
            cfg.qmake.spec = empty_string_to_none(spec);
        }
        if let Some(make) = qmake.make {
            cfg.qmake.make = empty_string_to_none(make);
        }
        if let Some(pro_file) = qmake.pro_file {
            cfg.qmake.pro_file =
                empty_path_to_none(pro_file).map(|path| absolutize(project_root, path));
        }
        if let Some(config_args) = qmake.config_args {
            cfg.qmake.config_args = config_args;
        }
    }

    for (name, raw_profile) in raw.profiles {
        let profile = default_profile(project_root, &name);
        cfg.profiles
            .insert(name, merge_profile(profile, raw_profile, project_root));
    }

    for (name, raw_test) in raw.tests {
        cfg.tests.insert(
            name,
            TestPreset {
                target: raw_test.target,
                regex: raw_test.regex,
                profile: raw_test.profile,
            },
        );
    }

    if let Some(diagnostics) = raw.diagnostics {
        if let Some(enabled) = diagnostics.enabled {
            cfg.diagnostics.enabled = enabled;
        }
        if let Some(max_log_bytes) = diagnostics.max_log_bytes {
            cfg.diagnostics.max_log_bytes = max_log_bytes;
        }
    }
}

fn apply_env(cfg: &mut ResolvedConfig, env: &BTreeMap<String, String>) {
    if let Some(cmake) = env.get("QTFLOW_CMAKE") {
        cfg.tools.cmake = cmake.clone();
    }
    if let Some(ctest) = env.get("QTFLOW_CTEST") {
        cfg.tools.ctest = ctest.clone();
    }
    if let Some(vsdevcmd) = env
        .get("QTFLOW_VSDEVCMD_BAT")
        .or_else(|| env.get("VSDEVCMD_BAT"))
    {
        cfg.msvc.vsdevcmd = empty_path_to_none(PathBuf::from(vsdevcmd));
    }
}

fn apply_cli(cfg: &mut ResolvedConfig, cli: &ConfigOverrides) {
    if let Some(cmake) = &cli.cmake {
        cfg.tools.cmake = cmake.clone();
    }
    if let Some(ctest) = &cli.ctest {
        cfg.tools.ctest = ctest.clone();
    }
    if let Some(qmake) = &cli.qmake {
        cfg.qmake.qmake = empty_string_to_none(qmake.clone());
    }
    if let Some(vsdevcmd) = &cli.vsdevcmd {
        cfg.msvc.vsdevcmd = Some(vsdevcmd.clone());
    }
    if let Some(msvc_enabled) = cli.msvc_enabled {
        cfg.msvc.enabled = msvc_enabled;
    }
}

fn default_profile(project_root: &Path, name: &str) -> Profile {
    Profile {
        preset: None,
        build_dir: absolutize(project_root, Path::new("out").join("build").join(name)),
        generator: None,
        config_name: None,
        configure_args: Vec::new(),
        cache_variables: BTreeMap::new(),
        build_args: Vec::new(),
        ctest_args: vec![DEFAULT_CTEST_ARG.to_string()],
        path_prepend: Vec::new(),
        env: BTreeMap::new(),
    }
}

fn merge_profile(mut profile: Profile, raw: RawProfile, project_root: &Path) -> Profile {
    if let Some(preset) = raw.preset {
        profile.preset = empty_string_to_none(preset);
    }
    if let Some(build_dir) = raw.build_dir {
        profile.build_dir = absolutize(project_root, build_dir);
    }
    if let Some(generator) = raw.generator {
        profile.generator = empty_string_to_none(generator);
    }
    if let Some(config_name) = raw.config_name {
        profile.config_name = empty_string_to_none(config_name);
    }
    if let Some(configure_args) = raw.configure_args {
        profile.configure_args = configure_args;
    }
    if let Some(cache_variables) = raw.cache_variables {
        profile.cache_variables = cache_variables;
    }
    if let Some(build_args) = raw.build_args {
        profile.build_args = build_args;
    }
    if let Some(ctest_args) = raw.ctest_args {
        profile.ctest_args = ctest_args;
    }
    if let Some(path_prepend) = raw.path_prepend {
        profile.path_prepend = path_prepend
            .into_iter()
            .map(|path| absolutize(project_root, path))
            .collect();
    }
    profile.env.extend(raw.env);
    profile
}

fn absolutize(project_root: &Path, path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

fn empty_string_to_none(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn empty_path_to_none(path: PathBuf) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::raw::{RawConfig, RawMsvc, RawProfile, RawTools};

    #[test]
    fn inferred_defaults_when_no_config_file_present() {
        let root = PathBuf::from("/repo");
        let cfg = resolve(None, &BTreeMap::new(), &ConfigOverrides::default(), &root);

        assert_eq!(cfg.default_profile, "debug");
        assert_eq!(cfg.active_profile, "debug");
        assert_eq!(cfg.build_system, BuildSystem::Auto);
        assert_eq!(cfg.tools.cmake, "cmake");
        assert_eq!(cfg.tools.ctest, "ctest");
        assert_eq!(cfg.qmake.qmake, None);
        assert_eq!(cfg.qmake.spec, None);
        assert_eq!(cfg.qmake.make, None);
        assert!(cfg.qmake.config_args.is_empty());
        assert!(cfg.warnings.is_empty());
        assert_eq!(cfg.msvc.arch, "x64");
        assert_eq!(
            cfg.profiles["debug"].build_dir,
            PathBuf::from("/repo/out/build/debug")
        );
        assert_eq!(cfg.profiles["debug"].preset.as_deref(), Some("Qt-Debug"));
        assert_eq!(
            cfg.profiles["debug"].ctest_args,
            vec!["--output-on-failure".to_string()]
        );
        assert_eq!(cfg.source, ConfigSource::Inferred);
    }

    #[test]
    fn file_defined_profile_without_preset_keeps_preset_absent() {
        let root = PathBuf::from("/repo");
        let raw = RawConfig {
            profiles: BTreeMap::from([(
                "debug".to_string(),
                RawProfile {
                    build_dir: Some(PathBuf::from("build")),
                    generator: Some("Ninja".to_string()),
                    ..RawProfile::default()
                },
            )]),
            ..RawConfig::default()
        };

        let cfg = resolve(
            Some(raw),
            &BTreeMap::new(),
            &ConfigOverrides::default(),
            &root,
        );

        assert_eq!(cfg.profiles["debug"].preset, None);
        assert_eq!(
            cfg.profiles["debug"].build_dir,
            PathBuf::from("/repo/build")
        );
        assert_eq!(cfg.profiles["debug"].generator.as_deref(), Some("Ninja"));
    }

    #[test]
    fn file_defined_profile_loads_cache_variables_and_path_prepend() {
        let root = PathBuf::from("/repo");
        let raw = RawConfig {
            profiles: BTreeMap::from([(
                "debug".to_string(),
                RawProfile {
                    cache_variables: Some(BTreeMap::from([(
                        "CMAKE_PREFIX_PATH".to_string(),
                        "C:/Qt".to_string(),
                    )])),
                    path_prepend: Some(vec![
                        PathBuf::from("tools/bin"),
                        PathBuf::from("/opt/qt/bin"),
                    ]),
                    ..RawProfile::default()
                },
            )]),
            ..RawConfig::default()
        };

        let cfg = resolve(
            Some(raw),
            &BTreeMap::new(),
            &ConfigOverrides::default(),
            &root,
        );

        assert_eq!(
            cfg.profiles["debug"].cache_variables,
            BTreeMap::from([("CMAKE_PREFIX_PATH".to_string(), "C:/Qt".to_string())])
        );
        assert_eq!(
            cfg.profiles["debug"].path_prepend,
            vec![
                PathBuf::from("/repo/tools/bin"),
                PathBuf::from("/opt/qt/bin")
            ]
        );
    }

    #[test]
    fn unknown_key_warnings_flow_to_resolved_config() {
        let root = PathBuf::from("/repo");
        let raw: RawConfig = toml::from_str(
            r#"
[profiles.debug]
cmake_args = ["-DOPT=ON"]
"#,
        )
        .expect("parse");

        let cfg = resolve(
            Some(raw),
            &BTreeMap::new(),
            &ConfigOverrides::default(),
            &root,
        );

        assert_eq!(
            cfg.warnings,
            vec!["unknown key 'cmake_args' in [profiles.debug] (ignored)".to_string()]
        );
    }

    #[test]
    fn file_defined_profile_with_preset_uses_file_value() {
        let root = PathBuf::from("/repo");
        let raw = RawConfig {
            profiles: BTreeMap::from([(
                "debug".to_string(),
                RawProfile {
                    preset: Some("X".to_string()),
                    ..RawProfile::default()
                },
            )]),
            ..RawConfig::default()
        };

        let cfg = resolve(
            Some(raw),
            &BTreeMap::new(),
            &ConfigOverrides::default(),
            &root,
        );

        assert_eq!(cfg.profiles["debug"].preset.as_deref(), Some("X"));
    }

    #[test]
    fn file_defined_qmake_config_resolves_paths_and_empty_auto_values() {
        let root = PathBuf::from("/repo");
        let raw = RawConfig {
            build_system: Some(RawBuildSystem::Qmake),
            qmake: Some(super::super::raw::RawQmake {
                qmake: Some("custom-qmake".to_string()),
                spec: Some("".to_string()),
                make: Some("nmake".to_string()),
                pro_file: Some(PathBuf::from("app/app.pro")),
                config_args: Some(vec!["-recursive".to_string()]),
                ..super::super::raw::RawQmake::default()
            }),
            ..RawConfig::default()
        };

        let cfg = resolve(
            Some(raw),
            &BTreeMap::new(),
            &ConfigOverrides::default(),
            &root,
        );

        assert_eq!(cfg.build_system, BuildSystem::Qmake);
        assert_eq!(cfg.qmake.qmake.as_deref(), Some("custom-qmake"));
        assert_eq!(cfg.qmake.spec, None);
        assert_eq!(cfg.qmake.make.as_deref(), Some("nmake"));
        assert_eq!(cfg.qmake.pro_file, Some(PathBuf::from("/repo/app/app.pro")));
        assert_eq!(cfg.qmake.config_args, vec!["-recursive".to_string()]);
    }

    #[test]
    fn no_config_file_still_uses_inferred_default_preset() {
        let root = PathBuf::from("/repo");
        let cfg = resolve(None, &BTreeMap::new(), &ConfigOverrides::default(), &root);

        assert_eq!(cfg.profiles["debug"].preset.as_deref(), Some("Qt-Debug"));
    }

    #[test]
    fn profile_config_name_is_optional_and_loaded_from_file() {
        let root = PathBuf::from("/repo");
        let cfg = resolve(None, &BTreeMap::new(), &ConfigOverrides::default(), &root);
        assert_eq!(cfg.profiles["debug"].config_name, None);

        let raw = RawConfig {
            profiles: BTreeMap::from([(
                "debug".to_string(),
                RawProfile {
                    config_name: Some("Debug".to_string()),
                    ..RawProfile::default()
                },
            )]),
            ..RawConfig::default()
        };

        let cfg = resolve(
            Some(raw),
            &BTreeMap::new(),
            &ConfigOverrides::default(),
            &root,
        );

        assert_eq!(cfg.profiles["debug"].config_name.as_deref(), Some("Debug"));
    }

    #[test]
    fn merge_precedence_cli_over_env_over_file_over_defaults() {
        let root = PathBuf::from("/repo");
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "file_profile".to_string(),
            RawProfile {
                preset: Some("File-Preset".to_string()),
                build_dir: Some(PathBuf::from("file-build")),
                ..RawProfile::default()
            },
        );
        let raw = RawConfig {
            default_profile: Some("file_profile".to_string()),
            tools: Some(RawTools {
                cmake: Some("file-cmake".to_string()),
                ctest: Some("file-ctest".to_string()),
                ninja: None,
                ..RawTools::default()
            }),
            msvc: Some(RawMsvc {
                vsdevcmd: Some(PathBuf::from("file-vsdevcmd.bat")),
                ..RawMsvc::default()
            }),
            profiles,
            ..RawConfig::default()
        };
        let env = BTreeMap::from([
            ("QTFLOW_PROFILE".to_string(), "env_profile".to_string()),
            ("QTFLOW_CMAKE".to_string(), "env-cmake".to_string()),
            ("QTFLOW_CTEST".to_string(), "env-ctest".to_string()),
            (
                "QTFLOW_VSDEVCMD_BAT".to_string(),
                "env-vsdevcmd.bat".to_string(),
            ),
        ]);
        let cli = ConfigOverrides {
            profile: Some("cli_profile".to_string()),
            cmake: Some("cli-cmake".to_string()),
            ctest: Some("cli-ctest".to_string()),
            qmake: Some("cli-qmake".to_string()),
            vsdevcmd: Some(PathBuf::from("cli-vsdevcmd.bat")),
            ..ConfigOverrides::default()
        };

        let cfg = resolve(Some(raw), &env, &cli, &root);

        assert_eq!(cfg.default_profile, "file_profile");
        assert_eq!(cfg.active_profile, "cli_profile");
        assert_eq!(cfg.tools.cmake, "cli-cmake");
        assert_eq!(cfg.tools.ctest, "cli-ctest");
        assert_eq!(cfg.qmake.qmake.as_deref(), Some("cli-qmake"));
        assert_eq!(cfg.msvc.vsdevcmd, Some(PathBuf::from("cli-vsdevcmd.bat")));
        assert_eq!(
            cfg.profiles["file_profile"].build_dir,
            PathBuf::from("/repo/file-build")
        );
        assert!(cfg.profiles.contains_key("cli_profile"));
    }

    #[test]
    fn env_overrides_file_when_cli_absent() {
        let root = PathBuf::from("/repo");
        let raw = RawConfig {
            default_profile: Some("file_profile".to_string()),
            tools: Some(RawTools {
                cmake: Some("file-cmake".to_string()),
                ctest: Some("file-ctest".to_string()),
                ninja: None,
                ..RawTools::default()
            }),
            ..RawConfig::default()
        };
        let env = BTreeMap::from([
            ("QTFLOW_PROFILE".to_string(), "env_profile".to_string()),
            ("QTFLOW_CMAKE".to_string(), "env-cmake".to_string()),
            ("QTFLOW_CTEST".to_string(), "env-ctest".to_string()),
        ]);

        let cfg = resolve(Some(raw), &env, &ConfigOverrides::default(), &root);

        assert_eq!(cfg.active_profile, "env_profile");
        assert_eq!(cfg.tools.cmake, "env-cmake");
        assert_eq!(cfg.tools.ctest, "env-ctest");
        assert_eq!(cfg.qmake.qmake, None);
    }

    #[test]
    fn env_config_path_sets_source_when_raw_is_loaded() {
        let root = PathBuf::from("/repo");
        let env = BTreeMap::from([(
            "QTFLOW_CONFIG".to_string(),
            "/repo/custom.qtflow.toml".to_string(),
        )]);

        let cfg = resolve(
            Some(RawConfig::default()),
            &env,
            &ConfigOverrides::default(),
            &root,
        );

        assert_eq!(
            cfg.source,
            ConfigSource::File(PathBuf::from("/repo/custom.qtflow.toml"))
        );
    }
}
