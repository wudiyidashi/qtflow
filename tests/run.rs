use assert_cmd::Command;
use predicates::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
enum CmakeStub {
    Success,
    Exit17,
    BuildDirMissing,
}

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

fn qmake_fixture_project(config: &str) -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("app.pro"),
        "TEMPLATE = app\nCONFIG += testcase\n",
    )
    .expect("write pro");
    std::fs::write(temp.path().join(".qtflow.toml"), config).expect("write config");
    temp
}

fn fixture_project_with_cmake_stub(stub: CmakeStub) -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.20)\n",
    )
    .expect("write CMakeLists");
    let cmake = write_cmake_stub(temp.path(), stub);
    std::fs::write(
        temp.path().join(".qtflow.toml"),
        format!(
            r#"
[tools]
cmake = {}

[msvc]
enabled = false

[profiles.debug]
preset = "unused"
build_dir = "out/build/debug"
"#,
            toml_basic_string(&cmake.to_string_lossy())
        ),
    )
    .expect("write config");
    temp
}

#[cfg(windows)]
fn write_cmake_stub(root: &Path, stub: CmakeStub) -> PathBuf {
    let path = root.join("qtflow-cmake-stub.cmd");
    let script = match stub {
        CmakeStub::Success => "@echo off\r\nexit /b 0\r\n",
        CmakeStub::Exit17 => "@echo off\r\nexit /b 17\r\n",
        CmakeStub::BuildDirMissing => {
            "@echo off\r\necho Error: could not load cache 1>&2\r\nexit /b 17\r\n"
        }
    };
    std::fs::write(&path, script).expect("write cmake stub");
    path
}

#[cfg(not(windows))]
fn write_cmake_stub(root: &Path, stub: CmakeStub) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = root.join("qtflow-cmake-stub");
    let script = match stub {
        CmakeStub::Success => "#!/bin/sh\nexit 0\n",
        CmakeStub::Exit17 => "#!/bin/sh\nexit 17\n",
        CmakeStub::BuildDirMissing => {
            "#!/bin/sh\necho 'Error: could not load cache' >&2\nexit 17\n"
        }
    };
    std::fs::write(&path, script).expect("write cmake stub");
    let mut permissions = std::fs::metadata(&path)
        .expect("cmake stub metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("make cmake stub executable");
    path
}

fn toml_basic_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn clean_qtflow() -> Command {
    let mut command = Command::cargo_bin("qtflow").expect("binary");
    command
        .env_remove("QTFLOW_CONFIG")
        .env_remove("QTFLOW_PROFILE")
        .env_remove("QTFLOW_CMAKE")
        .env_remove("QTFLOW_CTEST")
        .env_remove("QTFLOW_NINJA")
        .env_remove("QTFLOW_QMAKE")
        .env_remove("QTFLOW_VSDEVCMD_BAT")
        .env_remove("VSDEVCMD_BAT");
    command
}

#[cfg(windows)]
const DEPLOY_TOOL_NAME: &str = "windeployqt.exe";
#[cfg(target_os = "macos")]
const DEPLOY_TOOL_NAME: &str = "macdeployqt";

#[cfg(any(windows, target_os = "macos"))]
fn make_deploy_tool(root: &Path) -> PathBuf {
    let qt_bin = root.join("qt").join("bin");
    std::fs::create_dir_all(&qt_bin).expect("qt bin");
    let tool = qt_bin.join(DEPLOY_TOOL_NAME);
    std::fs::write(&tool, "").expect("deploy tool");
    tool
}

#[test]
fn build_executes_plan_and_success_exits_zero() {
    let temp = fixture_project_with_cmake_stub(CmakeStub::Success);

    clean_qtflow()
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["build", "--no-msvc-bootstrap"])
        .assert()
        .success();
}

#[cfg(any(windows, target_os = "macos"))]
#[test]
fn deploy_missing_executable_maps_to_exit_four() {
    let temp = fixture_project(
        r#"
[qt]
bin_dir = "qt/bin"

[msvc]
enabled = false

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
"#,
    );
    make_deploy_tool(temp.path());

    clean_qtflow()
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["deploy", "app"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains(
            "deploy executable for target 'app' was not found",
        ))
        .stderr(predicate::str::contains(
            "qtflow build <target> first or pass --exe",
        ));
}

#[cfg(target_os = "linux")]
#[test]
fn deploy_on_linux_maps_to_exit_three_with_clear_message() {
    let temp = fixture_project(
        r#"
[msvc]
enabled = false

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
"#,
    );

    clean_qtflow()
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["deploy", "app"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains(
            "Qt does not provide an official Linux deployment tool",
        ))
        .stderr(predicate::str::contains("linuxdeployqt"));
}

#[test]
fn build_non_zero_child_exit_maps_to_qtflow_exit_one() {
    let temp = fixture_project_with_cmake_stub(CmakeStub::Exit17);

    clean_qtflow()
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["build", "--no-msvc-bootstrap"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("exit code 17"));
}

#[test]
fn build_failure_prints_cmake_build_dir_missing_diagnostic() {
    let temp = fixture_project_with_cmake_stub(CmakeStub::BuildDirMissing);

    clean_qtflow()
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["build", "--no-msvc-bootstrap", "foo"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "diagnostic: CMake build directory is missing or not configured",
        ))
        .stderr(predicate::str::contains("Error: could not load cache"));
}

#[test]
fn build_failure_json_outputs_diagnostics_object() {
    let temp = fixture_project_with_cmake_stub(CmakeStub::BuildDirMissing);
    let output = clean_qtflow()
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["build", "--no-msvc-bootstrap", "foo", "--json"])
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

#[test]
fn missing_qmake_tool_maps_to_exit_three_with_qmake_diagnostic() {
    let temp = qmake_fixture_project(
        r#"
build_system = "qmake"

[qmake]
qmake = "definitely-missing-qmake-executable"

[msvc]
enabled = false

[profiles.debug]
build_dir = "out/build/debug"
"#,
    );

    clean_qtflow()
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["configure"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains(
            "diagnostic: qmake executable was not found",
        ));
}

#[test]
fn missing_qmake_make_tool_maps_to_exit_three_with_make_diagnostic() {
    let temp = qmake_fixture_project(
        r#"
build_system = "qmake"

[qmake]
make = "definitely-missing-qmake-make-tool"

[msvc]
enabled = false

[profiles.debug]
build_dir = "out/build/debug"
"#,
    );

    clean_qtflow()
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["check", "app"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains(
            "diagnostic: qmake make tool was not found",
        ));
}
