use assert_cmd::Command;
use predicates::prelude::*;

fn fixture_project(config: &str) -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.20)\n",
    )
    .expect("write CMakeLists");
    std::fs::write(temp.path().join(".qtflow.toml"), config).expect("write config");
    temp
}

fn clean_qtflow() -> Command {
    let mut command = Command::cargo_bin("qtflow").expect("binary");
    command
        .env_remove("QTFLOW_CONFIG")
        .env_remove("QTFLOW_PROFILE")
        .env_remove("QTFLOW_CMAKE")
        .env_remove("QTFLOW_CTEST")
        .env_remove("QTFLOW_VSDEVCMD_BAT")
        .env_remove("VSDEVCMD_BAT");
    command
}

#[cfg(windows)]
const SUCCESS_CONFIG: &str = r#"
[tools]
cmake = "cmd"

[msvc]
enabled = false

[profiles.debug]
preset = "unused"
build_dir = "out/build/debug"
build_args = ["/d", "/s", "/c", "exit 0"]
"#;

#[cfg(not(windows))]
const SUCCESS_CONFIG: &str = r#"
[tools]
cmake = "sh"

[msvc]
enabled = false

[profiles.debug]
preset = "unused"
build_dir = "out/build/debug"
build_args = ["-c", "exit 0"]
"#;

#[cfg(windows)]
const FAILURE_CONFIG: &str = r#"
[tools]
cmake = "cmd"

[msvc]
enabled = false

[profiles.debug]
preset = "unused"
build_dir = "out/build/debug"
build_args = ["/d", "/s", "/c", "exit 17"]
"#;

#[cfg(not(windows))]
const FAILURE_CONFIG: &str = r#"
[tools]
cmake = "sh"

[msvc]
enabled = false

[profiles.debug]
preset = "unused"
build_dir = "out/build/debug"
build_args = ["-c", "exit 17"]
"#;

#[test]
fn build_executes_plan_and_success_exits_zero() {
    let temp = fixture_project(SUCCESS_CONFIG);

    clean_qtflow()
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["build"])
        .assert()
        .success();
}

#[test]
fn build_non_zero_child_exit_maps_to_qtflow_exit_one() {
    let temp = fixture_project(FAILURE_CONFIG);

    clean_qtflow()
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["build"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("exit code 17"));
}

#[cfg(windows)]
const BUILD_DIR_MISSING_CONFIG: &str = r#"
[tools]
cmake = "cmd"

[msvc]
enabled = false

[profiles.debug]
preset = "unused"
build_dir = "out/build/debug"
build_args = ["/d", "/s", "/c", "echo Error: could not load cache 1>&2 && exit 17"]
"#;

#[cfg(not(windows))]
const BUILD_DIR_MISSING_CONFIG: &str = r#"
[tools]
cmake = "sh"

[msvc]
enabled = false

[profiles.debug]
preset = "unused"
build_dir = "out/build/debug"
build_args = ["-c", "echo 'Error: could not load cache' >&2; exit 17"]
"#;

#[test]
fn build_failure_prints_cmake_build_dir_missing_diagnostic() {
    let temp = fixture_project(BUILD_DIR_MISSING_CONFIG);

    clean_qtflow()
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["build", "foo"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "diagnostic: CMake build directory is missing or not configured",
        ))
        .stderr(predicate::str::contains("Error: could not load cache"));
}

#[test]
fn build_failure_json_outputs_diagnostics_object() {
    let temp = fixture_project(BUILD_DIR_MISSING_CONFIG);
    let output = clean_qtflow()
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["build", "foo", "--json"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    assert_eq!(json["exitCode"], 1);
    assert_eq!(json["diagnostics"][0]["code"], "cmake.build_dir_missing");
}

#[test]
fn missing_cmake_tool_maps_to_exit_three_with_diagnostic() {
    let temp = fixture_project(
        r#"
[tools]
cmake = "definitely-missing-cmake-executable"

[msvc]
enabled = false

[profiles.debug]
preset = "unused"
build_dir = "out/build/debug"
"#,
    );

    clean_qtflow()
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["build", "foo"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains(
            "diagnostic: CMake executable was not found",
        ));
}
