use std::path::{Path, PathBuf};

pub type PathExists<'a> = dyn Fn(&Path) -> bool + 'a;
pub type ProgramFinder<'a> = dyn Fn(&str) -> Option<PathBuf> + 'a;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPlatform {
    Windows,
    Macos,
    Linux,
    Other,
}

#[derive(Clone)]
pub struct DeployToolResolveInput<'a> {
    pub platform: HostPlatform,
    pub qt_bin_dir: Option<PathBuf>,
    pub qmake_path: Option<PathBuf>,
    pub path_exists: &'a PathExists<'a>,
    pub program_finder: &'a ProgramFinder<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeployToolResolution {
    Found {
        path: PathBuf,
        source: DeployToolSource,
        tool_name: String,
    },
    NotFound {
        tool_name: String,
        searched: Vec<DeployToolSearch>,
    },
    Unsupported {
        platform: HostPlatform,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployToolSource {
    QtBinDir,
    QmakeSibling,
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployToolSearch {
    pub source: DeployToolSource,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableResolveInput {
    pub project_root: PathBuf,
    pub build_dir: PathBuf,
    pub target: Option<String>,
    pub explicit_exe: Option<PathBuf>,
    pub exe_suffix: String,
    pub accept_app_bundle: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutableResolution {
    Found {
        path: PathBuf,
    },
    Missing {
        path: PathBuf,
        target: Option<String>,
    },
    MissingTarget,
}

impl HostPlatform {
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

impl<'a> DeployToolResolveInput<'a> {
    pub fn real(
        platform: HostPlatform,
        qt_bin_dir: Option<PathBuf>,
        qmake_path: Option<PathBuf>,
    ) -> Self {
        Self {
            platform,
            qt_bin_dir,
            qmake_path,
            path_exists: &Path::is_file,
            program_finder: &which_one,
        }
    }
}

pub fn resolve_deploy_tool(input: &DeployToolResolveInput<'_>) -> DeployToolResolution {
    let tool_names = tool_names(input.platform);
    if tool_names.is_empty() {
        return DeployToolResolution::Unsupported {
            platform: input.platform,
            message: unsupported_message(input.platform),
        };
    };
    let display_tool_name = tool_names[0];

    let mut searched = Vec::new();
    if let Some(bin_dir) = &input.qt_bin_dir {
        for tool_name in tool_names {
            let candidate = bin_dir.join(tool_name);
            if (input.path_exists)(&candidate) {
                return DeployToolResolution::Found {
                    path: candidate,
                    source: DeployToolSource::QtBinDir,
                    tool_name: (*tool_name).to_string(),
                };
            }
            searched.push(DeployToolSearch {
                source: DeployToolSource::QtBinDir,
                path: candidate,
            });
        }
    }

    if let Some(qmake_path) = &input.qmake_path {
        if let Some(parent) = qmake_path.parent() {
            for tool_name in tool_names {
                let candidate = parent.join(tool_name);
                if (input.path_exists)(&candidate) {
                    return DeployToolResolution::Found {
                        path: candidate,
                        source: DeployToolSource::QmakeSibling,
                        tool_name: (*tool_name).to_string(),
                    };
                }
                searched.push(DeployToolSearch {
                    source: DeployToolSource::QmakeSibling,
                    path: candidate,
                });
            }
        }
    }

    for tool_name in tool_names {
        if let Some(path) = (input.program_finder)(tool_name) {
            return DeployToolResolution::Found {
                path,
                source: DeployToolSource::Path,
                tool_name: (*tool_name).to_string(),
            };
        }
    }

    DeployToolResolution::NotFound {
        tool_name: display_tool_name.to_string(),
        searched,
    }
}

pub fn resolve_executable(input: &ExecutableResolveInput) -> ExecutableResolution {
    if let Some(exe) = &input.explicit_exe {
        let path = absolutize(&input.project_root, exe);
        return if executable_exists(&path, input.accept_app_bundle) {
            ExecutableResolution::Found { path }
        } else {
            ExecutableResolution::Missing { path, target: None }
        };
    }

    let Some(target) = input.target.as_deref().filter(|target| !target.is_empty()) else {
        return ExecutableResolution::MissingTarget;
    };

    for candidate in direct_candidates(input, target) {
        if executable_exists(&candidate, input.accept_app_bundle) {
            return ExecutableResolution::Found { path: candidate };
        }
    }

    if let Some(path) = shallow_find_target(&input.build_dir, target, input, 0) {
        return ExecutableResolution::Found { path };
    }

    ExecutableResolution::Missing {
        path: plausible_executable_path(input, target),
        target: Some(target.to_string()),
    }
}

pub fn plausible_executable_path(input: &ExecutableResolveInput, target: &str) -> PathBuf {
    input
        .build_dir
        .join("bin")
        .join(format!("{target}{}", input.exe_suffix))
}

fn direct_candidates(input: &ExecutableResolveInput, target: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let binary_name = format!("{target}{}", input.exe_suffix);
    candidates.push(input.build_dir.join("bin").join(&binary_name));
    if input.accept_app_bundle {
        candidates.push(input.build_dir.join("bin").join(format!("{target}.app")));
    }
    candidates.push(input.build_dir.join(&binary_name));
    if input.accept_app_bundle {
        candidates.push(input.build_dir.join(format!("{target}.app")));
    }
    candidates
}

fn shallow_find_target(
    dir: &Path,
    target: &str,
    input: &ExecutableResolveInput,
    depth: usize,
) -> Option<PathBuf> {
    if depth > 4 {
        return None;
    }

    let entries = sorted_entries(dir);
    for path in &entries {
        if is_named_executable(path, target, input) {
            return Some(path.clone());
        }
    }

    if depth == 4 {
        return None;
    }

    for path in entries {
        if should_descend(&path) {
            if let Some(found) = shallow_find_target(&path, target, input, depth + 1) {
                return Some(found);
            }
        }
    }

    None
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut entries = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    entries.sort();
    entries
}

fn should_descend(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    name != "CMakeFiles" && name != "_deps"
}

fn is_named_executable(path: &Path, target: &str, input: &ExecutableResolveInput) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if name == format!("{target}{}", input.exe_suffix) {
        return path.is_file();
    }
    input.accept_app_bundle && name == format!("{target}.app") && path.is_dir()
}

fn executable_exists(path: &Path, accept_app_bundle: bool) -> bool {
    if path.is_file() {
        return true;
    }
    accept_app_bundle
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
        && path.is_dir()
}

fn absolutize(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn tool_names(platform: HostPlatform) -> &'static [&'static str] {
    match platform {
        HostPlatform::Windows => &["windeployqt.exe", "windeployqt"],
        HostPlatform::Macos => &["macdeployqt"],
        HostPlatform::Linux | HostPlatform::Other => &[],
    }
}

fn unsupported_message(platform: HostPlatform) -> String {
    match platform {
        HostPlatform::Linux => "Qt does not provide an official Linux deployment tool. Use linuxdeployqt, package manually, or document the required Qt runtime dependencies.".to_string(),
        HostPlatform::Other => "Qt deployment is only supported by qtflow on Windows and macOS because Qt does not provide an official deployment tool for this host.".to_string(),
        HostPlatform::Windows | HostPlatform::Macos => String::new(),
    }
}

fn which_one(program: &str) -> Option<PathBuf> {
    which::which(program).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn resolve_fake(
        platform: HostPlatform,
        qt_bin_dir: Option<PathBuf>,
        qmake_path: Option<PathBuf>,
        path_hits: impl IntoIterator<Item = PathBuf>,
        path_tool: Option<PathBuf>,
    ) -> DeployToolResolution {
        let existing = BTreeSet::from_iter(path_hits);
        let path_exists = |path: &Path| existing.contains(path);
        let finder = move |_: &str| path_tool.clone();
        resolve_deploy_tool(&DeployToolResolveInput {
            platform,
            qt_bin_dir,
            qmake_path,
            path_exists: &path_exists,
            program_finder: &finder,
        })
    }

    #[test]
    fn qt_bin_dir_takes_precedence_for_windeployqt() {
        let tool = PathBuf::from("C:/Qt/bin/windeployqt.exe");
        let path_tool = PathBuf::from("C:/Other/windeployqt.exe");

        let resolution = resolve_fake(
            HostPlatform::Windows,
            Some(PathBuf::from("C:/Qt/bin")),
            Some(PathBuf::from("C:/Other/qmake.exe")),
            [tool.clone(), PathBuf::from("C:/Other/windeployqt.exe")],
            Some(path_tool),
        );

        assert_eq!(
            resolution,
            DeployToolResolution::Found {
                path: tool,
                source: DeployToolSource::QtBinDir,
                tool_name: "windeployqt.exe".to_string()
            }
        );
    }

    #[test]
    fn qmake_sibling_is_used_before_path() {
        let tool = PathBuf::from("/opt/Qt/bin/macdeployqt");
        let path_tool = PathBuf::from("/usr/bin/macdeployqt");

        let resolution = resolve_fake(
            HostPlatform::Macos,
            None,
            Some(PathBuf::from("/opt/Qt/bin/qmake")),
            [tool.clone()],
            Some(path_tool),
        );

        assert_eq!(
            resolution,
            DeployToolResolution::Found {
                path: tool,
                source: DeployToolSource::QmakeSibling,
                tool_name: "macdeployqt".to_string()
            }
        );
    }

    #[test]
    fn path_is_used_when_qt_hints_are_absent() {
        let tool = PathBuf::from("C:/Tools/windeployqt.exe");

        let resolution = resolve_fake(HostPlatform::Windows, None, None, [], Some(tool.clone()));

        assert_eq!(
            resolution,
            DeployToolResolution::Found {
                path: tool,
                source: DeployToolSource::Path,
                tool_name: "windeployqt.exe".to_string()
            }
        );
    }

    #[test]
    fn not_found_reports_tool_name_and_searched_paths() {
        let resolution = resolve_fake(
            HostPlatform::Windows,
            Some(PathBuf::from("C:/Qt/bin")),
            Some(PathBuf::from("C:/Other/bin/qmake.exe")),
            [],
            None,
        );

        assert_eq!(
            resolution,
            DeployToolResolution::NotFound {
                tool_name: "windeployqt.exe".to_string(),
                searched: vec![
                    DeployToolSearch {
                        source: DeployToolSource::QtBinDir,
                        path: PathBuf::from("C:/Qt/bin/windeployqt.exe")
                    },
                    DeployToolSearch {
                        source: DeployToolSource::QtBinDir,
                        path: PathBuf::from("C:/Qt/bin/windeployqt")
                    },
                    DeployToolSearch {
                        source: DeployToolSource::QmakeSibling,
                        path: PathBuf::from("C:/Other/bin/windeployqt.exe")
                    },
                    DeployToolSearch {
                        source: DeployToolSource::QmakeSibling,
                        path: PathBuf::from("C:/Other/bin/windeployqt")
                    }
                ]
            }
        );
    }

    #[test]
    fn linux_is_unsupported() {
        let resolution = resolve_fake(HostPlatform::Linux, None, None, [], None);

        assert!(matches!(
            resolution,
            DeployToolResolution::Unsupported {
                platform: HostPlatform::Linux,
                ..
            }
        ));
    }

    #[test]
    fn explicit_exe_overrides_target_search() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let explicit = root.join("custom").join("app.exe");
        std::fs::create_dir_all(explicit.parent().expect("parent")).expect("dir");
        std::fs::write(&explicit, "").expect("exe");
        let bin = root.join("build").join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        std::fs::write(bin.join("other.exe"), "").expect("other");

        let resolution = resolve_executable(&ExecutableResolveInput {
            project_root: root.to_path_buf(),
            build_dir: root.join("build"),
            target: Some("other".to_string()),
            explicit_exe: Some(PathBuf::from("custom/app.exe")),
            exe_suffix: ".exe".to_string(),
            accept_app_bundle: false,
        });

        assert_eq!(resolution, ExecutableResolution::Found { path: explicit });
    }

    #[test]
    fn bin_target_is_preferred() {
        let temp = tempfile::tempdir().expect("tempdir");
        let build_dir = temp.path().join("build");
        let bin = build_dir.join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let exe = bin.join("app.exe");
        std::fs::write(&exe, "").expect("exe");

        let resolution = resolve_executable(&ExecutableResolveInput {
            project_root: temp.path().to_path_buf(),
            build_dir,
            target: Some("app".to_string()),
            explicit_exe: None,
            exe_suffix: ".exe".to_string(),
            accept_app_bundle: false,
        });

        assert_eq!(resolution, ExecutableResolution::Found { path: exe });
    }

    #[test]
    fn shallow_search_skips_noise_dirs_and_finds_nested_exe() {
        let temp = tempfile::tempdir().expect("tempdir");
        let build_dir = temp.path().join("build");
        let noise = build_dir.join("_deps").join("pkg");
        let nested = build_dir.join("level1").join("level2");
        std::fs::create_dir_all(&noise).expect("noise");
        std::fs::create_dir_all(&nested).expect("nested");
        std::fs::write(noise.join("app.exe"), "").expect("noise exe");
        let exe = nested.join("app.exe");
        std::fs::write(&exe, "").expect("exe");

        let resolution = resolve_executable(&ExecutableResolveInput {
            project_root: temp.path().to_path_buf(),
            build_dir,
            target: Some("app".to_string()),
            explicit_exe: None,
            exe_suffix: ".exe".to_string(),
            accept_app_bundle: false,
        });

        assert_eq!(resolution, ExecutableResolution::Found { path: exe });
    }

    #[test]
    fn missing_target_returns_plausible_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let build_dir = temp.path().join("build");

        let resolution = resolve_executable(&ExecutableResolveInput {
            project_root: temp.path().to_path_buf(),
            build_dir: build_dir.clone(),
            target: Some("app".to_string()),
            explicit_exe: None,
            exe_suffix: ".exe".to_string(),
            accept_app_bundle: false,
        });

        assert_eq!(
            resolution,
            ExecutableResolution::Missing {
                path: build_dir.join("bin").join("app.exe"),
                target: Some("app".to_string())
            }
        );
    }
}
