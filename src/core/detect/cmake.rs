use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use std::collections::BTreeMap;

use super::Probe;

pub fn version(probe: &impl Probe, program: &str) -> Option<String> {
    probe.version(program)
}

#[derive(Debug)]
pub enum PresetListError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    InvalidShape {
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for PresetListError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(f, "failed to parse {}: {source}", path.display())
            }
            Self::InvalidShape { path, message } => {
                write!(f, "invalid {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for PresetListError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::InvalidShape { .. } => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CMakePresetsFile {
    #[serde(default)]
    configure_presets: Option<Vec<CMakePreset>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CMakePreset {
    name: Option<String>,
    #[serde(default)]
    hidden: Option<bool>,
    #[serde(default)]
    inherits: Option<PresetInherits>,
    #[serde(default)]
    binary_dir: Option<String>,
    #[serde(default)]
    cache_variables: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PresetInherits {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetInfo {
    pub name: String,
    pub binary_dir: Option<PathBuf>,
    pub build_type: Option<String>,
}

type PresetValueResolver =
    fn(&str, &BTreeMap<String, CMakePreset>, &mut Vec<String>) -> Option<String>;

pub fn list_presets(root: &Path) -> Result<Vec<String>, PresetListError> {
    Ok(list_preset_infos(root)?
        .into_iter()
        .map(|info| info.name)
        .collect())
}

pub fn list_preset_infos(root: &Path) -> Result<Vec<PresetInfo>, PresetListError> {
    let path = root.join("CMakePresets.json");
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(PresetListError::Read { path, source }),
    };

    let parsed: CMakePresetsFile =
        serde_json::from_str(&content).map_err(|source| PresetListError::Parse {
            path: path.clone(),
            source,
        })?;

    let configure_presets = parsed.configure_presets.unwrap_or_default();

    let mut by_name = BTreeMap::new();
    for preset in &configure_presets {
        if let Some(name) = preset.name.as_ref().filter(|name| !name.is_empty()) {
            by_name.insert(name.clone(), preset.clone());
        } else {
            return Err(PresetListError::InvalidShape {
                path: path.clone(),
                message: "configurePresets entries must have a non-empty name".to_string(),
            });
        }
    }

    let mut infos = Vec::new();
    for preset in &configure_presets {
        let name = preset
            .name
            .as_ref()
            .expect("preset names were validated")
            .clone();
        if preset.hidden.unwrap_or(false) {
            continue;
        }

        let binary_dir = resolve_binary_dir(&name, &by_name, &mut Vec::new())
            .map(|value| expand_binary_dir(root, &name, &value));
        let build_type = resolve_build_type(&name, &by_name, &mut Vec::new());
        infos.push(PresetInfo {
            name,
            binary_dir,
            build_type,
        });
    }

    Ok(infos)
}

fn resolve_binary_dir(
    name: &str,
    presets: &BTreeMap<String, CMakePreset>,
    stack: &mut Vec<String>,
) -> Option<String> {
    let preset = presets.get(name)?;
    resolve_inherited_value(
        name,
        preset.binary_dir.clone(),
        presets,
        stack,
        resolve_binary_dir,
    )
}

fn resolve_build_type(
    name: &str,
    presets: &BTreeMap<String, CMakePreset>,
    stack: &mut Vec<String>,
) -> Option<String> {
    let preset = presets.get(name)?;
    let local = preset
        .cache_variables
        .get("CMAKE_BUILD_TYPE")
        .and_then(cache_variable_string)
        .filter(|value| !value.is_empty());
    resolve_inherited_value(name, local, presets, stack, resolve_build_type)
}

fn resolve_inherited_value(
    name: &str,
    local: Option<String>,
    presets: &BTreeMap<String, CMakePreset>,
    stack: &mut Vec<String>,
    resolver: PresetValueResolver,
) -> Option<String> {
    if let Some(local) = local {
        return Some(local);
    }
    if stack.iter().any(|entry| entry == name) {
        return None;
    }

    let preset = presets.get(name)?;
    stack.push(name.to_string());
    let resolved = preset.inherits.as_ref().and_then(|inherits| {
        inherits
            .names()
            .iter()
            .find_map(|parent| resolver(parent, presets, stack))
    });
    stack.pop();
    resolved
}

fn cache_variable_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Object(object) => object.get("value").and_then(|value| match value {
            serde_json::Value::String(value) => Some(value.clone()),
            _ => None,
        }),
        _ => None,
    }
}

fn expand_binary_dir(root: &Path, preset_name: &str, value: &str) -> PathBuf {
    let source_dir = root.to_string_lossy().replace('\\', "/");
    let source_parent_dir = root
        .parent()
        .unwrap_or(root)
        .to_string_lossy()
        .replace('\\', "/");
    let expanded = value
        .replace("${sourceDir}", &source_dir)
        .replace("${presetName}", preset_name)
        .replace("${sourceParentDir}", &source_parent_dir);
    let path = PathBuf::from(expanded);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

impl PresetInherits {
    fn names(&self) -> Vec<String> {
        match self {
            Self::One(name) => vec![name.clone()],
            Self::Many(names) => names.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::detect::test_support::FakeProbe;

    #[test]
    fn version_uses_injected_probe() {
        let probe = FakeProbe::new([("custom-cmake", Some("cmake version 3.30.0".to_string()))]);

        assert_eq!(
            version(&probe, "custom-cmake").as_deref(),
            Some("cmake version 3.30.0")
        );
    }

    #[test]
    fn missing_presets_file_returns_empty_list() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert_eq!(
            list_presets(temp.path()).expect("presets"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn lists_configure_preset_names() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(
            temp.path().join("CMakePresets.json"),
            r#"{"version": 6, "configurePresets": [{"name": "Qt-Debug"}, {"name": "Qt-Release"}]}"#,
        )
        .expect("write presets");

        assert_eq!(
            list_presets(temp.path()).expect("presets"),
            vec!["Qt-Debug".to_string(), "Qt-Release".to_string()]
        );
    }

    #[test]
    fn preset_infos_expand_binary_dir_macros_exclude_hidden_and_inherit() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(
            temp.path().join("CMakePresets.json"),
            r#"{
                "version": 6,
                "configurePresets": [
                    {
                        "name": "base",
                        "hidden": true,
                        "binaryDir": "${sourceDir}/out/build/${presetName}",
                        "cacheVariables": {
                            "CMAKE_BUILD_TYPE": {"type": "STRING", "value": "Debug"}
                        }
                    },
                    {"name": "Qt-Debug", "inherits": "base"},
                    {"name": "Qt-Release", "binaryDir": "${sourceDir}/build-release", "cacheVariables": {"CMAKE_BUILD_TYPE": "Release"}}
                ]
            }"#,
        )
        .expect("write presets");

        let infos = list_preset_infos(temp.path()).expect("preset infos");

        assert_eq!(
            infos
                .iter()
                .map(|info| info.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Qt-Debug", "Qt-Release"]
        );
        let debug = infos
            .iter()
            .find(|info| info.name == "Qt-Debug")
            .expect("debug");
        assert_eq!(
            debug.binary_dir.as_deref(),
            Some(temp.path().join("out/build/Qt-Debug").as_path())
        );
        assert_eq!(debug.build_type.as_deref(), Some("Debug"));
    }

    #[test]
    fn invalid_presets_file_returns_warning_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("CMakePresets.json"), "{").expect("write presets");

        let err = list_presets(temp.path()).expect_err("invalid presets");

        assert!(err.to_string().contains("failed to parse"));
    }
}
