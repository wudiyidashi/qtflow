use std::path::PathBuf;

use serde::Serialize;

use crate::core::config::model::QtConfig;
use crate::core::path::serialize_optional_path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QtHints {
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_path"
    )]
    pub root: Option<PathBuf>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_path"
    )]
    pub bin_dir: Option<PathBuf>,
}

impl QtHints {
    pub fn is_empty(&self) -> bool {
        self.root.is_none() && self.bin_dir.is_none()
    }
}

pub fn hints(config: &QtConfig) -> QtHints {
    QtHints {
        root: config.root.clone(),
        bin_dir: config.bin_dir.clone(),
    }
}
