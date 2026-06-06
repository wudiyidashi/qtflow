use std::fs;
use std::path::{Path, PathBuf};

use crate::error::QtflowError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Cmake,
    Qmake,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuildSystemPreference {
    #[default]
    Auto,
    Cmake,
    Qmake,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectContext {
    pub root: PathBuf,
    pub kind: ProjectKind,
    pub project_file: PathBuf,
    pub cmake_lists: Option<PathBuf>,
    pub presets_file: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
}

pub fn discover_root(start: &Path) -> Result<ProjectContext, QtflowError> {
    discover_root_with_preference(start, BuildSystemPreference::Auto)
}

pub fn discover_root_with_preference(
    start: &Path,
    preference: BuildSystemPreference,
) -> Result<ProjectContext, QtflowError> {
    let mut current = normalize_start(start)?;

    loop {
        let cmake_lists = current.join("CMakeLists.txt");
        let pro_files = pro_files(&current);
        let found = match preference {
            BuildSystemPreference::Auto => {
                if cmake_lists.is_file() {
                    Some((ProjectKind::Cmake, cmake_lists.clone()))
                } else {
                    primary_pro_file(&current, pro_files).map(|path| (ProjectKind::Qmake, path))
                }
            }
            BuildSystemPreference::Cmake => cmake_lists
                .is_file()
                .then(|| (ProjectKind::Cmake, cmake_lists.clone())),
            BuildSystemPreference::Qmake => {
                primary_pro_file(&current, pro_files).map(|path| (ProjectKind::Qmake, path))
            }
        };

        if let Some((kind, project_file)) = found {
            return Ok(project_context(current, kind, project_file));
        }

        if !current.pop() {
            return Err(QtflowError::ProjectRootNotFound {
                start: start.to_path_buf(),
            });
        }
    }
}

fn project_context(root: PathBuf, kind: ProjectKind, project_file: PathBuf) -> ProjectContext {
    let cmake_lists = (kind == ProjectKind::Cmake).then(|| project_file.clone());
    let presets_file = (kind == ProjectKind::Cmake)
        .then(|| root.join("CMakePresets.json"))
        .filter(|path| path.is_file());
    let config_file = locate_project_config(&root);

    ProjectContext {
        root,
        kind,
        project_file,
        cmake_lists,
        presets_file,
        config_file,
    }
}

pub fn locate_project_config(root: &Path) -> Option<PathBuf> {
    [".qtflow.toml", "qtflow.toml"]
        .into_iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
}

fn pro_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("pro"))
            })
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    files.sort();
    files
}

fn primary_pro_file(dir: &Path, mut files: Vec<PathBuf>) -> Option<PathBuf> {
    if files.is_empty() {
        return None;
    }
    if files.len() == 1 {
        return files.pop();
    }

    let dir_stem = dir.file_name().and_then(|name| name.to_str()).unwrap_or("");
    files.sort_by(|left, right| {
        // qmake projects often keep a top-level TEMPLATE=subdirs umbrella next to the
        // app project. Prefer app-like files, then a stem matching the directory, then
        // a stable lexical order so multi-.pro discovery is deterministic.
        pro_rank(left, dir_stem).cmp(&pro_rank(right, dir_stem))
    });
    files.into_iter().next()
}

fn pro_rank(path: &Path, dir_stem: &str) -> (u8, u8, String) {
    let template = read_pro_template(path);
    let app_like_rank = match template.as_deref() {
        Some("subdirs") => 1,
        _ => 0,
    };
    let stem_rank = if path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case(dir_stem))
    {
        0
    } else {
        1
    };
    (
        app_like_rank,
        stem_rank,
        path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default(),
    )
}

fn read_pro_template(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    content.lines().find_map(|line| {
        let line = line.split('#').next().unwrap_or("").trim();
        let rest = line.strip_prefix("TEMPLATE")?.trim_start();
        let rest = rest.strip_prefix('=').or_else(|| rest.strip_prefix("+="))?;
        rest.split_whitespace()
            .next()
            .map(|value| value.trim_matches('"').trim_matches('\'').to_lowercase())
            .filter(|value| !value.is_empty())
    })
}

fn normalize_start(start: &Path) -> Result<PathBuf, QtflowError> {
    if start.exists() {
        let canonical = start
            .canonicalize()
            .map_err(|_| QtflowError::ProjectRootNotFound {
                start: start.to_path_buf(),
            })?;
        let canonical = strip_windows_verbatim_prefix(canonical);
        if canonical.is_file() {
            Ok(canonical
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or(canonical))
        } else {
            Ok(canonical)
        }
    } else {
        Err(QtflowError::ProjectRootNotFound {
            start: start.to_path_buf(),
        })
    }
}

#[cfg(windows)]
fn strip_windows_verbatim_prefix(path: PathBuf) -> PathBuf {
    let path = path.to_string_lossy();
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        PathBuf::from(path.as_ref())
    }
}

#[cfg(not(windows))]
fn strip_windows_verbatim_prefix(path: PathBuf) -> PathBuf {
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonicalize(path: &Path) -> PathBuf {
        strip_windows_verbatim_prefix(std::fs::canonicalize(path).expect("canonical path"))
    }

    #[test]
    fn discovers_project_root_from_child_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("CMakeLists.txt"), "").expect("CMakeLists");
        std::fs::write(temp.path().join("qtflow.toml"), "").expect("config");
        std::fs::write(temp.path().join("CMakePresets.json"), "{}").expect("presets");
        let child = temp.path().join("src").join("nested");
        std::fs::create_dir_all(&child).expect("child dir");

        let ctx = discover_root(&child).expect("project context");

        assert_eq!(canonicalize(&ctx.root), canonicalize(temp.path()));
        assert_eq!(ctx.kind, ProjectKind::Cmake);
        assert_eq!(
            canonicalize(&ctx.project_file),
            canonicalize(&temp.path().join("CMakeLists.txt"))
        );
        assert_eq!(
            ctx.cmake_lists.as_deref().map(canonicalize),
            Some(canonicalize(&temp.path().join("CMakeLists.txt")))
        );
        assert_eq!(
            ctx.config_file.as_deref().map(canonicalize),
            Some(canonicalize(&temp.path().join("qtflow.toml")))
        );
        assert_eq!(
            ctx.presets_file.as_deref().map(canonicalize),
            Some(canonicalize(&temp.path().join("CMakePresets.json")))
        );
    }

    #[test]
    fn missing_project_root_maps_to_exit_code_4() {
        let temp = tempfile::tempdir().expect("tempdir");

        let err = discover_root(temp.path()).expect_err("missing CMakeLists");

        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn discovers_qmake_project_with_single_pro_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("app.pro"), "SOURCES += main.cpp\n").expect("pro");

        let ctx = discover_root(temp.path()).expect("project context");

        assert_eq!(ctx.kind, ProjectKind::Qmake);
        assert_eq!(
            canonicalize(&ctx.project_file),
            canonicalize(&temp.path().join("app.pro"))
        );
        assert_eq!(ctx.cmake_lists, None);
    }

    #[test]
    fn cmake_takes_precedence_when_both_exist_in_same_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("CMakeLists.txt"), "").expect("CMakeLists");
        std::fs::write(temp.path().join("app.pro"), "").expect("pro");

        let ctx = discover_root(temp.path()).expect("project context");

        assert_eq!(ctx.kind, ProjectKind::Cmake);
        assert_eq!(
            canonicalize(&ctx.project_file),
            canonicalize(&temp.path().join("CMakeLists.txt"))
        );
    }

    #[test]
    fn multiple_pro_files_prefer_app_over_subdirs() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("umbrella.pro"), "TEMPLATE = subdirs\n").expect("subdirs");
        std::fs::write(temp.path().join("app.pro"), "TEMPLATE = app\n").expect("app");

        let ctx = discover_root(temp.path()).expect("project context");

        assert_eq!(ctx.kind, ProjectKind::Qmake);
        assert_eq!(
            canonicalize(&ctx.project_file),
            canonicalize(&temp.path().join("app.pro"))
        );
    }

    #[test]
    fn forced_cmake_ignores_qmake_project_in_child_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("CMakeLists.txt"), "").expect("CMakeLists");
        let child = temp.path().join("examples");
        std::fs::create_dir_all(&child).expect("child");
        std::fs::write(child.join("app.pro"), "").expect("pro");

        let ctx = discover_root_with_preference(&child, BuildSystemPreference::Cmake)
            .expect("project context");

        assert_eq!(ctx.kind, ProjectKind::Cmake);
        assert_eq!(canonicalize(&ctx.root), canonicalize(temp.path()));
    }
}
