use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub type PathExists<'a> = dyn Fn(&Path) -> bool + 'a;
pub type VsWhereRunner<'a> = dyn Fn(&Path) -> Option<String> + 'a;

#[derive(Clone)]
pub struct MsvcResolveInput<'a> {
    pub cli_path: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub config_path: Option<PathBuf>,
    pub path_exists: &'a PathExists<'a>,
    pub run_vswhere: Option<&'a VsWhereRunner<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VsDevCmdResolution {
    Found {
        path: PathBuf,
        source: VsDevCmdSource,
    },
    NotFound {
        searched: Vec<VsDevCmdSource>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VsDevCmdSource {
    Cli,
    EnvQtflow,
    EnvCompat,
    Config,
    VsInstallDir,
    VsWhere,
    KnownPath,
}

impl<'a> MsvcResolveInput<'a> {
    pub fn real(
        cli_path: Option<PathBuf>,
        env: BTreeMap<String, String>,
        config_path: Option<PathBuf>,
        run_vswhere: Option<&'a VsWhereRunner<'a>>,
    ) -> Self {
        Self {
            cli_path,
            env,
            config_path,
            path_exists: &Path::is_file,
            run_vswhere,
        }
    }
}

pub fn resolve_vsdevcmd(input: &MsvcResolveInput<'_>) -> VsDevCmdResolution {
    let mut searched = Vec::new();

    if let Some(path) = &input.cli_path {
        if first_existing(
            std::iter::once(path.clone()),
            VsDevCmdSource::Cli,
            input.path_exists,
            &mut searched,
        )
        .is_some()
        {
            return found(path.clone(), VsDevCmdSource::Cli);
        }
    }

    if let Some(path) = env_get(&input.env, "QTFLOW_VSDEVCMD_BAT").map(PathBuf::from) {
        if first_existing(
            std::iter::once(path.clone()),
            VsDevCmdSource::EnvQtflow,
            input.path_exists,
            &mut searched,
        )
        .is_some()
        {
            return found(path, VsDevCmdSource::EnvQtflow);
        }
    }

    if let Some(path) = env_get(&input.env, "VSDEVCMD_BAT").map(PathBuf::from) {
        if first_existing(
            std::iter::once(path.clone()),
            VsDevCmdSource::EnvCompat,
            input.path_exists,
            &mut searched,
        )
        .is_some()
        {
            return found(path, VsDevCmdSource::EnvCompat);
        }
    }

    if let Some(path) = &input.config_path {
        if first_existing(
            std::iter::once(path.clone()),
            VsDevCmdSource::Config,
            input.path_exists,
            &mut searched,
        )
        .is_some()
        {
            return found(path.clone(), VsDevCmdSource::Config);
        }
    }

    if let Some(vs_install_dir) = env_get(&input.env, "VSINSTALLDIR") {
        let path = PathBuf::from(vs_install_dir)
            .join("Common7")
            .join("Tools")
            .join("VsDevCmd.bat");
        if first_existing(
            std::iter::once(path.clone()),
            VsDevCmdSource::VsInstallDir,
            input.path_exists,
            &mut searched,
        )
        .is_some()
        {
            return found(path, VsDevCmdSource::VsInstallDir);
        }
    }

    let vswhere_candidates = vswhere_candidates(&input.env);
    for vswhere in vswhere_candidates {
        if !(input.path_exists)(&vswhere) && vswhere.components().count() > 1 {
            continue;
        }

        let Some(run_vswhere) = input.run_vswhere else {
            continue;
        };
        let Some(install_path) = run_vswhere(&vswhere) else {
            continue;
        };
        let install_path = install_path.trim();
        if install_path.is_empty() {
            continue;
        }

        let path = PathBuf::from(install_path)
            .join("Common7")
            .join("Tools")
            .join("VsDevCmd.bat");
        if (input.path_exists)(&path) {
            return found(path, VsDevCmdSource::VsWhere);
        }
    }
    searched.push(VsDevCmdSource::VsWhere);

    if let Some(path) = first_existing(
        known_vsdevcmd_candidates(&input.env),
        VsDevCmdSource::KnownPath,
        input.path_exists,
        &mut searched,
    ) {
        return found(path, VsDevCmdSource::KnownPath);
    }

    VsDevCmdResolution::NotFound { searched }
}

pub fn known_vsdevcmd_candidates(env: &BTreeMap<String, String>) -> Vec<PathBuf> {
    let roots = ["ProgramFiles", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(|name| env_get(env, name))
        .collect::<Vec<_>>();

    let versions = ["2022", "18"];
    let editions = ["Community", "Professional", "Enterprise", "BuildTools"];
    let mut candidates = Vec::new();

    for root in roots {
        for version in versions {
            for edition in editions {
                candidates.push(
                    PathBuf::from(root)
                        .join("Microsoft Visual Studio")
                        .join(version)
                        .join(edition)
                        .join("Common7")
                        .join("Tools")
                        .join("VsDevCmd.bat"),
                );
            }
        }
    }

    candidates
}

fn found(path: PathBuf, source: VsDevCmdSource) -> VsDevCmdResolution {
    VsDevCmdResolution::Found { path, source }
}

fn first_existing(
    candidates: impl IntoIterator<Item = PathBuf>,
    source: VsDevCmdSource,
    path_exists: &PathExists<'_>,
    searched: &mut Vec<VsDevCmdSource>,
) -> Option<PathBuf> {
    let mut saw_candidate = false;
    for path in candidates {
        saw_candidate = true;
        if path_exists(&path) {
            return Some(path);
        }
    }

    if saw_candidate {
        searched.push(source);
    }
    None
}

fn vswhere_candidates(env: &BTreeMap<String, String>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(program_files_x86) = env_get(env, "ProgramFiles(x86)") {
        candidates.push(
            PathBuf::from(program_files_x86)
                .join("Microsoft Visual Studio")
                .join("Installer")
                .join("vswhere.exe"),
        );
    }
    candidates.push(PathBuf::from("vswhere"));
    candidates
}

fn env_get<'a>(env: &'a BTreeMap<String, String>, key: &str) -> Option<&'a String> {
    env.get(key).or_else(|| {
        env.iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn resolve_fake(
        env: BTreeMap<String, String>,
        cli_path: Option<PathBuf>,
        config_path: Option<PathBuf>,
        existing: &BTreeSet<PathBuf>,
        vswhere_output: Option<&str>,
    ) -> VsDevCmdResolution {
        let path_exists = |path: &Path| path == Path::new("vswhere") || existing.contains(path);
        let run_vswhere = |_: &Path| vswhere_output.map(str::to_string);
        MsvcResolveInput {
            cli_path,
            env,
            config_path,
            path_exists: &path_exists,
            run_vswhere: Some(&run_vswhere),
        }
        .resolve()
    }

    trait ResolveForTest {
        fn resolve(&self) -> VsDevCmdResolution;
    }

    impl ResolveForTest for MsvcResolveInput<'_> {
        fn resolve(&self) -> VsDevCmdResolution {
            resolve_vsdevcmd(self)
        }
    }

    #[test]
    fn cli_beats_env_config_vsinstall_vswhere_and_known_path() {
        let cli = PathBuf::from("C:/cli/VsDevCmd.bat");
        let env_qtflow = PathBuf::from("C:/env-qtflow/VsDevCmd.bat");
        let env = BTreeMap::from([
            (
                "QTFLOW_VSDEVCMD_BAT".to_string(),
                env_qtflow.to_string_lossy().to_string(),
            ),
            (
                "VSDEVCMD_BAT".to_string(),
                "C:/env/VsDevCmd.bat".to_string(),
            ),
            ("VSINSTALLDIR".to_string(), "C:/vsinstall".to_string()),
            ("ProgramFiles".to_string(), "C:/Program Files".to_string()),
            (
                "ProgramFiles(x86)".to_string(),
                "C:/Program Files (x86)".to_string(),
            ),
        ]);
        let config = PathBuf::from("C:/config/VsDevCmd.bat");
        let existing = BTreeSet::from([
            cli.clone(),
            env_qtflow,
            config.clone(),
            PathBuf::from("C:/vsinstall/Common7/Tools/VsDevCmd.bat"),
            PathBuf::from("C:/vswhere/Common7/Tools/VsDevCmd.bat"),
        ]);

        let result = resolve_fake(
            env,
            Some(cli.clone()),
            Some(config),
            &existing,
            Some("C:/vswhere"),
        );

        assert_eq!(
            result,
            VsDevCmdResolution::Found {
                path: cli,
                source: VsDevCmdSource::Cli
            }
        );
    }

    #[test]
    fn env_qtflow_beats_compat_config_vsinstall_vswhere_and_known_path() {
        let env_qtflow = PathBuf::from("C:/env-qtflow/VsDevCmd.bat");
        let env = BTreeMap::from([
            (
                "QTFLOW_VSDEVCMD_BAT".to_string(),
                env_qtflow.to_string_lossy().to_string(),
            ),
            (
                "VSDEVCMD_BAT".to_string(),
                "C:/env/VsDevCmd.bat".to_string(),
            ),
            ("VSINSTALLDIR".to_string(), "C:/vsinstall".to_string()),
        ]);
        let existing = BTreeSet::from([
            env_qtflow.clone(),
            PathBuf::from("C:/env/VsDevCmd.bat"),
            PathBuf::from("C:/vsinstall/Common7/Tools/VsDevCmd.bat"),
        ]);

        let result = resolve_fake(
            env,
            None,
            Some(PathBuf::from("C:/config/VsDevCmd.bat")),
            &existing,
            Some("C:/vswhere"),
        );

        assert_eq!(
            result,
            VsDevCmdResolution::Found {
                path: env_qtflow,
                source: VsDevCmdSource::EnvQtflow
            }
        );
    }

    #[test]
    fn env_compat_beats_config_vsinstall_vswhere_and_known_path() {
        let compat = PathBuf::from("C:/env/VsDevCmd.bat");
        let env = BTreeMap::from([
            (
                "VSDEVCMD_BAT".to_string(),
                compat.to_string_lossy().to_string(),
            ),
            ("VSINSTALLDIR".to_string(), "C:/vsinstall".to_string()),
        ]);
        let existing = BTreeSet::from([
            compat.clone(),
            PathBuf::from("C:/vsinstall/Common7/Tools/VsDevCmd.bat"),
        ]);

        let result = resolve_fake(
            env,
            None,
            Some(PathBuf::from("C:/config/VsDevCmd.bat")),
            &existing,
            Some("C:/vswhere"),
        );

        assert_eq!(
            result,
            VsDevCmdResolution::Found {
                path: compat,
                source: VsDevCmdSource::EnvCompat
            }
        );
    }

    #[test]
    fn config_beats_vsinstall_vswhere_and_known_path() {
        let config = PathBuf::from("C:/config/VsDevCmd.bat");
        let env = BTreeMap::from([("VSINSTALLDIR".to_string(), "C:/vsinstall".to_string())]);
        let existing = BTreeSet::from([
            config.clone(),
            PathBuf::from("C:/vsinstall/Common7/Tools/VsDevCmd.bat"),
        ]);

        let result = resolve_fake(
            env,
            None,
            Some(config.clone()),
            &existing,
            Some("C:/vswhere"),
        );

        assert_eq!(
            result,
            VsDevCmdResolution::Found {
                path: config,
                source: VsDevCmdSource::Config
            }
        );
    }

    #[test]
    fn vsinstalldir_beats_vswhere_and_known_path() {
        let path = PathBuf::from("C:/vsinstall/Common7/Tools/VsDevCmd.bat");
        let env = BTreeMap::from([("VSINSTALLDIR".to_string(), "C:/vsinstall".to_string())]);
        let existing = BTreeSet::from([
            path.clone(),
            PathBuf::from("C:/vswhere/Common7/Tools/VsDevCmd.bat"),
        ]);

        let result = resolve_fake(env, None, None, &existing, Some("C:/vswhere"));

        assert_eq!(
            result,
            VsDevCmdResolution::Found {
                path,
                source: VsDevCmdSource::VsInstallDir
            }
        );
    }

    #[test]
    fn vswhere_beats_known_path() {
        let path = PathBuf::from("C:/vswhere/Common7/Tools/VsDevCmd.bat");
        let known = PathBuf::from(
            "C:/Program Files/Microsoft Visual Studio/2022/Community/Common7/Tools/VsDevCmd.bat",
        );
        let env = BTreeMap::from([("ProgramFiles".to_string(), "C:/Program Files".to_string())]);
        let existing = BTreeSet::from([path.clone(), known]);

        let result = resolve_fake(env, None, None, &existing, Some("C:/vswhere"));

        assert_eq!(
            result,
            VsDevCmdResolution::Found {
                path,
                source: VsDevCmdSource::VsWhere
            }
        );
    }

    #[test]
    fn known_path_is_last_resolution_source() {
        let path = PathBuf::from(
            "C:/Program Files/Microsoft Visual Studio/2022/Community/Common7/Tools/VsDevCmd.bat",
        );
        let env = BTreeMap::from([("ProgramFiles".to_string(), "C:/Program Files".to_string())]);
        let existing = BTreeSet::from([path.clone()]);

        let result = resolve_fake(env, None, None, &existing, None);

        assert_eq!(
            result,
            VsDevCmdResolution::Found {
                path,
                source: VsDevCmdSource::KnownPath
            }
        );
    }

    #[test]
    fn not_found_returns_sources_that_were_searched() {
        let env = BTreeMap::from([
            (
                "QTFLOW_VSDEVCMD_BAT".to_string(),
                "C:/missing/qtflow.bat".to_string(),
            ),
            (
                "VSDEVCMD_BAT".to_string(),
                "C:/missing/compat.bat".to_string(),
            ),
            ("VSINSTALLDIR".to_string(), "C:/missing/vs".to_string()),
            ("ProgramFiles".to_string(), "C:/Program Files".to_string()),
        ]);
        let existing = BTreeSet::new();

        let result = resolve_fake(
            env,
            Some(PathBuf::from("C:/missing/cli.bat")),
            Some(PathBuf::from("C:/missing/config.bat")),
            &existing,
            Some("C:/missing/vswhere"),
        );

        assert_eq!(
            result,
            VsDevCmdResolution::NotFound {
                searched: vec![
                    VsDevCmdSource::Cli,
                    VsDevCmdSource::EnvQtflow,
                    VsDevCmdSource::EnvCompat,
                    VsDevCmdSource::Config,
                    VsDevCmdSource::VsInstallDir,
                    VsDevCmdSource::VsWhere,
                    VsDevCmdSource::KnownPath,
                ]
            }
        );
    }

    #[test]
    fn known_candidates_include_supported_years_editions_and_roots() {
        let env = BTreeMap::from([
            ("ProgramFiles".to_string(), "C:/Program Files".to_string()),
            (
                "ProgramFiles(x86)".to_string(),
                "C:/Program Files (x86)".to_string(),
            ),
        ]);

        let candidates = known_vsdevcmd_candidates(&env);

        assert!(candidates.contains(&PathBuf::from(
            "C:/Program Files/Microsoft Visual Studio/2022/Community/Common7/Tools/VsDevCmd.bat"
        )));
        assert!(candidates.contains(&PathBuf::from(
            "C:/Program Files (x86)/Microsoft Visual Studio/18/BuildTools/Common7/Tools/VsDevCmd.bat"
        )));
        assert_eq!(candidates.len(), 16);
    }
}
