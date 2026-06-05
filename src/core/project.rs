use std::path::{Path, PathBuf};

use crate::error::QtflowError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectContext {
    pub root: PathBuf,
    pub cmake_lists: PathBuf,
    pub presets_file: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
}

pub fn discover_root(start: &Path) -> Result<ProjectContext, QtflowError> {
    let mut current = normalize_start(start)?;

    loop {
        let cmake_lists = current.join("CMakeLists.txt");
        if cmake_lists.is_file() {
            let presets_file = current
                .join("CMakePresets.json")
                .is_file()
                .then(|| current.join("CMakePresets.json"));
            let config_file = locate_project_config(&current);

            return Ok(ProjectContext {
                root: current,
                cmake_lists,
                presets_file,
                config_file,
            });
        }

        if !current.pop() {
            return Err(QtflowError::ProjectRootNotFound {
                start: start.to_path_buf(),
            });
        }
    }
}

pub fn locate_project_config(root: &Path) -> Option<PathBuf> {
    [".qtflow.toml", "qtflow.toml"]
        .into_iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
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

    #[test]
    fn discovers_project_root_from_child_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("CMakeLists.txt"), "").expect("CMakeLists");
        std::fs::write(temp.path().join("qtflow.toml"), "").expect("config");
        std::fs::write(temp.path().join("CMakePresets.json"), "{}").expect("presets");
        let child = temp.path().join("src").join("nested");
        std::fs::create_dir_all(&child).expect("child dir");

        let ctx = discover_root(&child).expect("project context");

        assert_eq!(ctx.root, temp.path());
        assert_eq!(ctx.config_file, Some(ctx.root.join("qtflow.toml")));
        assert_eq!(ctx.presets_file, Some(ctx.root.join("CMakePresets.json")));
    }

    #[test]
    fn missing_project_root_maps_to_exit_code_4() {
        let temp = tempfile::tempdir().expect("tempdir");

        let err = discover_root(temp.path()).expect_err("missing CMakeLists");

        assert_eq!(err.exit_code(), 4);
    }
}
