use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::Probe;

pub type PathExists<'a> = dyn Fn(&Path) -> bool + 'a;
pub type PathCandidates<'a> = dyn Fn(&str) -> Vec<PathBuf> + 'a;
pub type ProgramFinder<'a> = dyn Fn(&str) -> Option<PathBuf> + 'a;

#[derive(Clone)]
pub struct QmakeResolveInput<'a> {
    pub configured: Option<String>,
    pub env: BTreeMap<String, String>,
    pub path_exists: &'a PathExists<'a>,
    pub path_candidates: &'a PathCandidates<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QmakeResolution {
    pub path: Option<PathBuf>,
    pub source: Option<QmakeSource>,
    pub rejected: Vec<QmakeRejectedCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QmakeSource {
    Config,
    EnvQtflow,
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QmakeRejectedCandidate {
    pub path: PathBuf,
    pub reason: QmakeRejectionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QmakeRejectionReason {
    Conda,
}

#[derive(Clone)]
pub struct MakeToolResolveInput<'a> {
    pub configured: Option<String>,
    pub spec: String,
    pub is_windows: bool,
    pub program_finder: &'a ProgramFinder<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MakeToolResolution {
    pub tool: String,
    pub source: MakeToolSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MakeToolSource {
    Config,
    Path,
    Fallback,
}

impl<'a> QmakeResolveInput<'a> {
    pub fn real(configured: Option<String>, env: BTreeMap<String, String>) -> Self {
        Self {
            configured,
            env,
            path_exists: &Path::is_file,
            path_candidates: &which_all,
        }
    }
}

impl<'a> MakeToolResolveInput<'a> {
    pub fn real(configured: Option<String>, spec: String, is_windows: bool) -> Self {
        Self {
            configured,
            spec,
            is_windows,
            program_finder: &which_one,
        }
    }
}

pub fn version(probe: &impl Probe, program: &str) -> Option<String> {
    probe.version(program)
}

pub fn resolve_qmake(input: &QmakeResolveInput<'_>) -> QmakeResolution {
    if let Some(configured) = input.configured.as_ref().filter(|value| !value.is_empty()) {
        if let Some(path) = explicit_program(configured, input.path_exists) {
            return QmakeResolution {
                path: Some(path),
                source: Some(QmakeSource::Config),
                rejected: Vec::new(),
            };
        }
    }

    if let Some(env_qmake) = env_get(&input.env, "QTFLOW_QMAKE").filter(|value| !value.is_empty()) {
        if let Some(path) = explicit_program(env_qmake, input.path_exists) {
            return QmakeResolution {
                path: Some(path),
                source: Some(QmakeSource::EnvQtflow),
                rejected: Vec::new(),
            };
        }
    }

    let mut rejected = Vec::new();
    let mut viable = Vec::new();
    for candidate in (input.path_candidates)("qmake") {
        if !(input.path_exists)(&candidate) {
            continue;
        }
        if is_conda_path(&candidate) {
            rejected.push(QmakeRejectedCandidate {
                path: candidate,
                reason: QmakeRejectionReason::Conda,
            });
        } else {
            viable.push(candidate);
        }
    }

    let path = viable
        .iter()
        .find(|path| has_qt_install_marker(path))
        .cloned()
        .or_else(|| viable.into_iter().next());
    let source = path.as_ref().map(|_| QmakeSource::Path);

    QmakeResolution {
        path,
        source,
        rejected,
    }
}

pub fn resolve_make_tool(input: &MakeToolResolveInput<'_>) -> MakeToolResolution {
    if let Some(configured) = input.configured.as_ref().filter(|value| !value.is_empty()) {
        return MakeToolResolution {
            tool: configured.clone(),
            source: MakeToolSource::Config,
        };
    }

    let spec = input.spec.to_lowercase();
    let candidates = if spec.contains("msvc") {
        &["nmake", "jom"][..]
    } else if spec.contains("mingw") {
        &["mingw32-make", "make"][..]
    } else if input.is_windows {
        &["nmake", "jom", "mingw32-make", "make"][..]
    } else {
        &["make"][..]
    };

    if let Some(path) = candidates
        .iter()
        .find_map(|program| (input.program_finder)(program))
    {
        return MakeToolResolution {
            tool: path.to_string_lossy().replace('\\', "/"),
            source: MakeToolSource::Path,
        };
    }

    MakeToolResolution {
        tool: default_make_tool(&spec, input.is_windows).to_string(),
        source: MakeToolSource::Fallback,
    }
}

pub fn default_spec(is_windows: bool, msvc_enabled: bool) -> &'static str {
    if is_windows && msvc_enabled {
        "win32-msvc"
    } else if cfg!(target_os = "macos") {
        "macx-clang"
    } else {
        "linux-g++"
    }
}

fn default_make_tool(spec: &str, is_windows: bool) -> &'static str {
    if spec.contains("mingw") {
        "mingw32-make"
    } else if spec.contains("msvc") || is_windows {
        "nmake"
    } else {
        "make"
    }
}

fn explicit_program(value: &str, path_exists: &PathExists<'_>) -> Option<PathBuf> {
    let path = PathBuf::from(value);
    let _ = path_exists;
    Some(path)
}

fn which_all(program: &str) -> Vec<PathBuf> {
    which::which_all(program)
        .map(|candidates| candidates.collect())
        .unwrap_or_default()
}

fn which_one(program: &str) -> Option<PathBuf> {
    which::which(program).ok()
}

fn is_conda_path(path: &Path) -> bool {
    let path = path.to_string_lossy().to_lowercase();
    path.contains("anaconda") || path.contains("miniconda") || path.contains("conda")
}

fn has_qt_install_marker(path: &Path) -> bool {
    let path = path.to_string_lossy().to_lowercase();
    path.contains("qt") || path.contains("msvc") || path.contains("mingw")
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
        configured: Option<String>,
        env: BTreeMap<String, String>,
        candidates: Vec<PathBuf>,
        existing: BTreeSet<PathBuf>,
    ) -> QmakeResolution {
        let path_exists = |path: &Path| path.components().count() <= 1 || existing.contains(path);
        let path_candidates = move |_: &str| candidates.clone();
        resolve_qmake(&QmakeResolveInput {
            configured,
            env,
            path_exists: &path_exists,
            path_candidates: &path_candidates,
        })
    }

    #[test]
    fn path_search_skips_conda_and_prefers_qt_candidate() {
        let conda = PathBuf::from("C:/Users/me/Anaconda3/Library/bin/qmake.exe");
        let plain = PathBuf::from("C:/tools/bin/qmake.exe");
        let qt = PathBuf::from("C:/Qt/6.8.0/msvc2022_64/bin/qmake.exe");
        let existing = BTreeSet::from([conda.clone(), plain, qt.clone()]);

        let resolution = resolve_fake(
            None,
            BTreeMap::new(),
            vec![
                conda.clone(),
                PathBuf::from("C:/tools/bin/qmake.exe"),
                qt.clone(),
            ],
            existing,
        );

        assert_eq!(resolution.path, Some(qt));
        assert_eq!(resolution.source, Some(QmakeSource::Path));
        assert_eq!(
            resolution.rejected,
            vec![QmakeRejectedCandidate {
                path: conda,
                reason: QmakeRejectionReason::Conda
            }]
        );
    }

    #[test]
    fn configured_qmake_overrides_path_search() {
        let configured = PathBuf::from("C:/custom/qmake.exe");
        let conda = PathBuf::from("C:/Anaconda3/Library/bin/qmake.exe");
        let qt = PathBuf::from("C:/Qt/6.8.0/msvc2022_64/bin/qmake.exe");
        let existing = BTreeSet::from([configured.clone(), conda.clone(), qt]);

        let resolution = resolve_fake(
            Some(configured.to_string_lossy().to_string()),
            BTreeMap::new(),
            vec![conda],
            existing,
        );

        assert_eq!(resolution.path, Some(configured));
        assert_eq!(resolution.source, Some(QmakeSource::Config));
        assert!(resolution.rejected.is_empty());
    }

    #[test]
    fn env_qmake_overrides_path_search_when_config_absent() {
        let env_qmake = PathBuf::from("C:/env/qmake.exe");
        let qt = PathBuf::from("C:/Qt/6.8.0/msvc2022_64/bin/qmake.exe");
        let existing = BTreeSet::from([env_qmake.clone(), qt.clone()]);
        let env = BTreeMap::from([(
            "QTFLOW_QMAKE".to_string(),
            env_qmake.to_string_lossy().to_string(),
        )]);

        let resolution = resolve_fake(None, env, vec![qt], existing);

        assert_eq!(resolution.path, Some(env_qmake));
        assert_eq!(resolution.source, Some(QmakeSource::EnvQtflow));
    }

    #[test]
    fn make_tool_uses_nmake_for_msvc_and_make_for_gnu() {
        let finder =
            |program: &str| (program == "nmake").then(|| PathBuf::from("C:/VS/Tools/nmake.exe"));

        let msvc = resolve_make_tool(&MakeToolResolveInput {
            configured: None,
            spec: "win32-msvc".to_string(),
            is_windows: true,
            program_finder: &finder,
        });
        assert_eq!(msvc.tool, "C:/VS/Tools/nmake.exe");
        assert_eq!(msvc.source, MakeToolSource::Path);

        let none = |_: &str| None;
        let gnu = resolve_make_tool(&MakeToolResolveInput {
            configured: None,
            spec: "linux-g++".to_string(),
            is_windows: false,
            program_finder: &none,
        });
        assert_eq!(gnu.tool, "make");
        assert_eq!(gnu.source, MakeToolSource::Fallback);
    }
}
