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
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawMsvc {
    pub enabled: Option<bool>,
    pub arch: Option<String>,
    pub host_arch: Option<String>,
    pub vsdevcmd: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawQt {
    pub root: Option<PathBuf>,
    pub bin_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawQmake {
    pub qmake: Option<String>,
    pub spec: Option<String>,
    pub make: Option<String>,
    pub pro_file: Option<PathBuf>,
    #[serde(default)]
    pub config_args: Option<Vec<String>>,
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
    pub build_args: Option<Vec<String>>,
    #[serde(default)]
    pub ctest_args: Option<Vec<String>>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawTestPreset {
    pub target: Option<String>,
    pub regex: Option<String>,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawDiagnostics {
    pub enabled: Option<bool>,
    pub max_log_bytes: Option<usize>,
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
    }
}
