use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedConfig {
    pub default_profile: String,
    pub active_profile: String,
    pub tools: Tools,
    pub msvc: MsvcConfig,
    pub qt: QtConfig,
    pub profiles: BTreeMap<String, Profile>,
    pub tests: BTreeMap<String, TestPreset>,
    pub diagnostics: DiagnosticsConfig,
    pub source: ConfigSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tools {
    pub cmake: String,
    pub ctest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ninja: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MsvcConfig {
    pub enabled: bool,
    pub arch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_arch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vsdevcmd: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QtConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    pub build_dir: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_name: Option<String>,
    pub configure_args: Vec<String>,
    pub build_args: Vec<String>,
    pub ctest_args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestPreset {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsConfig {
    pub enabled: bool,
    pub max_log_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigSource {
    Inferred,
    File(PathBuf),
}

impl ConfigSource {
    pub fn display(&self) -> String {
        match self {
            Self::Inferred => "<inferred>".to_string(),
            Self::File(path) => path.display().to_string(),
        }
    }
}
