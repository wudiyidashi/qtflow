use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawConfig {
    pub default_profile: Option<String>,
    pub build_system: Option<RawBuildSystem>,
    #[serde(default)]
    pub tools: Option<RawTools>,
    #[serde(default)]
    pub msvc: Option<RawMsvc>,
    #[serde(default)]
    pub qt: Option<RawQt>,
    #[serde(default)]
    pub qmake: Option<RawQmake>,
    #[serde(default)]
    pub profiles: BTreeMap<String, RawProfile>,
    #[serde(default)]
    pub tests: BTreeMap<String, RawTestPreset>,
    #[serde(default)]
    pub diagnostics: Option<RawDiagnostics>,
    #[serde(default, flatten)]
    pub unknown: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RawBuildSystem {
    Auto,
    Cmake,
    Qmake,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawTools {
    pub cmake: Option<String>,
    pub ctest: Option<String>,
    pub ninja: Option<String>,
    #[serde(default, flatten)]
    pub unknown: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawMsvc {
    pub enabled: Option<bool>,
    pub arch: Option<String>,
    pub host_arch: Option<String>,
    pub vsdevcmd: Option<PathBuf>,
    #[serde(default, flatten)]
    pub unknown: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawQt {
    pub root: Option<PathBuf>,
    pub bin_dir: Option<PathBuf>,
    #[serde(default, flatten)]
    pub unknown: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawQmake {
    pub qmake: Option<String>,
    pub spec: Option<String>,
    pub make: Option<String>,
    pub pro_file: Option<PathBuf>,
    #[serde(default)]
    pub config_args: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub unknown: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawProfile {
    pub preset: Option<String>,
    pub build_dir: Option<PathBuf>,
    pub generator: Option<String>,
    pub config_name: Option<String>,
    #[serde(default)]
    pub configure_args: Option<Vec<String>>,
    #[serde(default)]
    pub cache_variables: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub path_prepend: Option<Vec<PathBuf>>,
    #[serde(default)]
    pub build_args: Option<Vec<String>>,
    #[serde(default)]
    pub ctest_args: Option<Vec<String>>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default, flatten)]
    pub unknown: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawTestPreset {
    pub target: Option<String>,
    pub regex: Option<String>,
    pub profile: Option<String>,
    #[serde(default, flatten)]
    pub unknown: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawDiagnostics {
    pub enabled: Option<bool>,
    pub max_log_bytes: Option<usize>,
    #[serde(default, flatten)]
    pub unknown: BTreeMap<String, toml::Value>,
}

impl RawConfig {
    pub fn unknown_key_warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        extend_unknown_warnings(&mut warnings, "", &self.unknown);
        if let Some(tools) = &self.tools {
            extend_unknown_warnings(&mut warnings, "tools", &tools.unknown);
        }
        if let Some(msvc) = &self.msvc {
            extend_unknown_warnings(&mut warnings, "msvc", &msvc.unknown);
        }
        if let Some(qt) = &self.qt {
            extend_unknown_warnings(&mut warnings, "qt", &qt.unknown);
        }
        if let Some(qmake) = &self.qmake {
            extend_unknown_warnings(&mut warnings, "qmake", &qmake.unknown);
        }
        for (name, profile) in &self.profiles {
            extend_unknown_warnings(&mut warnings, &format!("profiles.{name}"), &profile.unknown);
        }
        for (name, test) in &self.tests {
            extend_unknown_warnings(&mut warnings, &format!("tests.{name}"), &test.unknown);
        }
        if let Some(diagnostics) = &self.diagnostics {
            extend_unknown_warnings(&mut warnings, "diagnostics", &diagnostics.unknown);
        }
        warnings
    }
}

fn extend_unknown_warnings(
    warnings: &mut Vec<String>,
    table: &str,
    unknown: &BTreeMap<String, toml::Value>,
) {
    for key in unknown.keys() {
        if table.is_empty() {
            warnings.push(format!("unknown key '{key}' at top level (ignored)"));
        } else {
            warnings.push(format!("unknown key '{key}' in [{table}] (ignored)"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_config_example() {
        let input = r#"
default_profile = "debug"
build_system = "auto"

[tools]
cmake = "cmake"
ctest = "ctest"
ninja = "ninja"

[msvc]
enabled = true
arch = "x64"
host_arch = "x64"
vsdevcmd = ""

[qt]
root = ""
bin_dir = ""

[qmake]
qmake = ""
spec = ""
make = ""
pro_file = ""
config_args = ["-recursive"]

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
generator = "Ninja"
config_name = "Debug"
configure_args = []
cache_variables = {}
path_prepend = []
build_args = []
ctest_args = ["--output-on-failure"]

[profiles.debug.env]
QT_LOGGING_RULES = "qt.qml=false"

[profiles.release]
preset = "Qt-Release"
build_dir = "out/build/release"
generator = "Ninja"
config_name = "Release"
configure_args = []
cache_variables = {}
path_prepend = []
build_args = []
ctest_args = ["--output-on-failure"]

[tests.route_dispatcher]
target = "route_dispatcher_request_build_test"
regex = "route_dispatcher_request_build_test"
profile = "debug"

[diagnostics]
enabled = true
max_log_bytes = 200000
"#;

        let parsed: RawConfig = toml::from_str(input).expect("full example parses");

        assert_eq!(parsed.default_profile.as_deref(), Some("debug"));
        assert!(matches!(parsed.build_system, Some(RawBuildSystem::Auto)));
        assert_eq!(
            parsed
                .qmake
                .as_ref()
                .and_then(|qmake| qmake.config_args.as_ref())
                .expect("qmake args"),
            &vec!["-recursive".to_string()]
        );
        assert!(parsed.profiles.contains_key("debug"));
        assert_eq!(
            parsed.profiles["debug"].config_name.as_deref(),
            Some("Debug")
        );
        assert!(parsed.tests.contains_key("route_dispatcher"));
        assert!(parsed.unknown_key_warnings().is_empty());
    }

    #[test]
    fn collects_unknown_key_warnings_with_table_paths() {
        let input = r#"
unknown_root = true

[tools]
extra_tool = "x"

[profiles.debug]
build_dir = "build"
cmake_args = ["-DOPT=ON"]

[profiles.debug.env]
FREE_FORM = "ok"

[tests.smoke]
regex = "smoke"
typo = true
"#;

        let parsed: RawConfig = toml::from_str(input).expect("config parses");

        assert_eq!(
            parsed.unknown_key_warnings(),
            vec![
                "unknown key 'unknown_root' at top level (ignored)".to_string(),
                "unknown key 'extra_tool' in [tools] (ignored)".to_string(),
                "unknown key 'cmake_args' in [profiles.debug] (ignored)".to_string(),
                "unknown key 'typo' in [tests.smoke] (ignored)".to_string()
            ]
        );
    }
}
