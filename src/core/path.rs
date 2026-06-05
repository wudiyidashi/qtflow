use std::path::Path;

use serde::Serializer;

pub fn path_to_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn serialize_path<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&path_to_slash(path))
}

pub fn serialize_optional_path<S>(
    path: &Option<std::path::PathBuf>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match path {
        Some(path) => serializer.serialize_some(&path_to_slash(path)),
        None => serializer.serialize_none(),
    }
}
