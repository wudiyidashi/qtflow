use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub type ProgramFinder<'a> = dyn Fn(&str) -> Option<PathBuf> + 'a;

#[derive(Clone)]
pub struct NinjaResolveInput<'a> {
    pub configured: Option<String>,
    pub env: BTreeMap<String, String>,
    pub program_finder: &'a ProgramFinder<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NinjaResolution {
    pub path: Option<PathBuf>,
    pub source: Option<NinjaSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NinjaSource {
    Config,
    EnvQtflow,
    Path,
}

impl<'a> NinjaResolveInput<'a> {
    pub fn real(configured: Option<String>, env: BTreeMap<String, String>) -> Self {
        Self {
            configured,
            env,
            program_finder: &which_one,
        }
    }
}

pub fn resolve_ninja(input: &NinjaResolveInput<'_>) -> NinjaResolution {
    if let Some(configured) = input.configured.as_ref().filter(|value| !value.is_empty()) {
        return NinjaResolution {
            path: Some(resolve_explicit_program(configured, input.program_finder)),
            source: Some(NinjaSource::Config),
        };
    }

    if let Some(env_ninja) = env_get(&input.env, "QTFLOW_NINJA").filter(|value| !value.is_empty()) {
        return NinjaResolution {
            path: Some(resolve_explicit_program(env_ninja, input.program_finder)),
            source: Some(NinjaSource::EnvQtflow),
        };
    }

    match (input.program_finder)("ninja") {
        Some(path) => NinjaResolution {
            path: Some(path),
            source: Some(NinjaSource::Path),
        },
        None => NinjaResolution {
            path: None,
            source: None,
        },
    }
}

fn resolve_explicit_program(value: &str, program_finder: &ProgramFinder<'_>) -> PathBuf {
    let path = PathBuf::from(value);
    if is_path_like(&path) {
        path
    } else {
        program_finder(value).unwrap_or(path)
    }
}

fn is_path_like(path: &Path) -> bool {
    path.is_absolute() || path.components().count() > 1
}

fn which_one(program: &str) -> Option<PathBuf> {
    which::which(program).ok()
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

    #[test]
    fn configured_ninja_wins_over_env_and_path() {
        let finder = |_: &str| Some(PathBuf::from("path/ninja"));
        let input = NinjaResolveInput {
            configured: Some("config/ninja".to_string()),
            env: BTreeMap::from([("QTFLOW_NINJA".to_string(), "env/ninja".to_string())]),
            program_finder: &finder,
        };

        let resolution = resolve_ninja(&input);

        assert_eq!(resolution.path, Some(PathBuf::from("config/ninja")));
        assert_eq!(resolution.source, Some(NinjaSource::Config));
    }

    #[test]
    fn configured_ninja_name_is_resolved_through_finder() {
        let finder =
            |program: &str| (program == "custom-ninja").then(|| PathBuf::from("C:/bin/ninja.exe"));
        let input = NinjaResolveInput {
            configured: Some("custom-ninja".to_string()),
            env: BTreeMap::new(),
            program_finder: &finder,
        };

        let resolution = resolve_ninja(&input);

        assert_eq!(resolution.path, Some(PathBuf::from("C:/bin/ninja.exe")));
        assert_eq!(resolution.source, Some(NinjaSource::Config));
    }

    #[test]
    fn env_ninja_wins_over_path() {
        let finder = |_: &str| Some(PathBuf::from("path/ninja"));
        let input = NinjaResolveInput {
            configured: None,
            env: BTreeMap::from([("QTFLOW_NINJA".to_string(), "env/ninja".to_string())]),
            program_finder: &finder,
        };

        let resolution = resolve_ninja(&input);

        assert_eq!(resolution.path, Some(PathBuf::from("env/ninja")));
        assert_eq!(resolution.source, Some(NinjaSource::EnvQtflow));
    }

    #[test]
    fn falls_back_to_path_search() {
        let finder = |program: &str| (program == "ninja").then(|| PathBuf::from("path/ninja"));
        let input = NinjaResolveInput {
            configured: None,
            env: BTreeMap::new(),
            program_finder: &finder,
        };

        let resolution = resolve_ninja(&input);

        assert_eq!(resolution.path, Some(PathBuf::from("path/ninja")));
        assert_eq!(resolution.source, Some(NinjaSource::Path));
    }
}
