use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use crate::core::path::serialize_path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandPlan {
    #[serde(serialize_with = "serialize_path")]
    pub project_root: PathBuf,
    pub profile: String,
    pub steps: Vec<CommandStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandStep {
    pub label: String,
    #[serde(serialize_with = "serialize_path")]
    pub cwd: PathBuf,
    pub program: String,
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap: Option<EnvironmentBootstrap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EnvironmentBootstrap {
    Msvc {
        #[serde(serialize_with = "serialize_path")]
        vsdevcmd: PathBuf,
        arch: String,
        #[serde(rename = "hostArch", skip_serializing_if = "Option::is_none")]
        host_arch: Option<String>,
    },
}
