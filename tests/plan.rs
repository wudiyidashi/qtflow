use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

fn fixture_project() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.20)\n",
    )
    .expect("write CMakeLists");
    std::fs::write(
        temp.path().join(".qtflow.toml"),
        r#"
default_profile = "debug"

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
ctest_args = []

[msvc]
enabled = false
"#,
    )
    .expect("write config");
    temp
}

fn fixture_project_with_config(config: &str) -> tempfile::TempDir {
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
    std::fs::write(temp.path().join("app.pro"), "TEMPLATE = app\n").expect("write pro");
    std::fs::write(temp.path().join(".qtflow.toml"), config).expect("write config");
    temp
}

fn qtflow_json(args: &[&str]) -> Value {
    let output = Command::cargo_bin("qtflow")
        .expect("binary")
        .env_remove("QTFLOW_CONFIG")
        .env_remove("QTFLOW_PROFILE")
        .env_remove("QTFLOW_CMAKE")
        .env_remove("QTFLOW_CTEST")
        .env_remove("QTFLOW_NINJA")
        .env_remove("QTFLOW_QMAKE")
        .env_remove("QTFLOW_VSDEVCMD_BAT")
        .env_remove("VSDEVCMD_BAT")
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    serde_json::from_slice(&output).expect("valid JSON")
}

#[test]
fn qmake_plan_build_json_uses_make_tool_shape() {
    let temp = qmake_fixture_project(
        r#"
build_system = "qmake"

[qmake]
qmake = "qmake-stub"
spec = "linux-g++"
make = "make"

[msvc]
enabled = false

[profiles.debug]
build_dir = "out/build/debug"
build_args = ["VERBOSE=1"]
"#,
    );
    let project = temp.path().to_str().unwrap();

    let json = qtflow_json(&["--project", project, "plan", "build", "app", "--json"]);
    let root = json["projectRoot"].as_str().expect("project root");

    assert_eq!(json["steps"][0]["label"], "build");
    assert_eq!(json["steps"][0]["program"], "make");
    assert_eq!(json["steps"][0]["cwd"], root);
    assert_eq!(
        json["steps"][0]["args"],
        serde_json::json!(["-C", format!("{root}/out/build/debug"), "app", "VERBOSE=1"])
    );
}

#[test]
fn qmake_plan_configure_json_uses_qmake_arg_shape() {
    let temp = qmake_fixture_project(
        r#"
build_system = "qmake"

[qmake]
qmake = "qmake-stub"
spec = "linux-g++"
make = "make"
config_args = ["-recursive"]

[msvc]
enabled = false

[profiles.debug]
build_dir = "out/build/debug"
"#,
    );
    let project = temp.path().to_str().unwrap();

    let json = qtflow_json(&["--project", project, "plan", "configure", "--json"]);
    let root = json["projectRoot"].as_str().expect("project root");

    assert_eq!(json["steps"][0]["program"], "qmake-stub");
    assert_eq!(
        json["steps"][0]["args"],
        serde_json::json!([
            "-o",
            format!("{root}/out/build/debug/Makefile"),
            format!("{root}/app.pro"),
            "-spec",
            "linux-g++",
            "CONFIG+=debug",
            "-recursive",
            "-after",
            format!("DESTDIR={root}/out/build/debug/bin")
        ])
    );
}

#[test]
fn qmake_plan_build_with_nmake_uses_build_dir_cwd_without_dash_c() {
    let temp = qmake_fixture_project(
        r#"
build_system = "qmake"

[qmake]
qmake = "qmake-stub"
spec = "win32-msvc"
make = "nmake"

[msvc]
enabled = false

[profiles.debug]
build_dir = "out/build/debug"
"#,
    );
    let project = temp.path().to_str().unwrap();

    let json = qtflow_json(&["--project", project, "plan", "build", "app", "--json"]);
    let root = json["projectRoot"].as_str().expect("project root");

    assert_eq!(json["steps"][0]["program"], "nmake");
    assert_eq!(json["steps"][0]["cwd"], format!("{root}/out/build/debug"));
    assert_eq!(json["steps"][0]["args"], serde_json::json!(["app"]));
}

#[test]
fn qmake_plan_test_is_not_yet_supported() {
    let temp = qmake_fixture_project(
        r#"
build_system = "qmake"

[msvc]
enabled = false

[profiles.debug]
build_dir = "out/build/debug"
"#,
    );

    Command::cargo_bin("qtflow")
        .expect("binary")
        .env_remove("QTFLOW_CONFIG")
        .env_remove("QTFLOW_PROFILE")
        .env_remove("QTFLOW_CMAKE")
        .env_remove("QTFLOW_CTEST")
        .env_remove("QTFLOW_NINJA")
        .env_remove("QTFLOW_QMAKE")
        .env_remove("QTFLOW_VSDEVCMD_BAT")
        .env_remove("VSDEVCMD_BAT")
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["plan", "test", "smoke"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "qmake test/check not yet supported (coming in next phase)",
        ));
}

#[test]
fn build_system_cmake_forces_cmake_when_child_has_pro_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.20)\n",
    )
    .expect("write CMakeLists");
    std::fs::write(
        temp.path().join(".qtflow.toml"),
        r#"
build_system = "cmake"

[msvc]
enabled = false

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
"#,
    )
    .expect("write config");
    let child = temp.path().join("example");
    std::fs::create_dir_all(&child).expect("child");
    std::fs::write(child.join("app.pro"), "TEMPLATE = app\n").expect("write pro");

    let json = qtflow_json(&[
        "--project",
        child.to_str().unwrap(),
        "plan",
        "build",
        "app",
        "--json",
    ]);

    assert_eq!(json["steps"][0]["program"], "cmake");
}

#[test]
fn plan_check_json_matches_contract_shape() {
    let temp = fixture_project();
    let project = temp.path().to_str().unwrap();

    let json = qtflow_json(&["--project", project, "plan", "check", "foo", "--json"]);
    let root = json["projectRoot"].as_str().expect("project root");
    let build_dir = format!("{root}/out/build/debug");

    assert_eq!(json["profile"], "debug");
    assert_eq!(json["steps"].as_array().expect("steps").len(), 2);
    assert_eq!(json["steps"][0]["label"], "build");
    assert_eq!(json["steps"][0]["cwd"], root);
    assert_eq!(json["steps"][0]["program"], "cmake");
    assert_eq!(
        json["steps"][0]["args"],
        serde_json::json!(["--build", build_dir, "--target", "foo"])
    );
    assert_eq!(json["steps"][1]["label"], "test");
    assert_eq!(json["steps"][1]["cwd"], root);
    assert_eq!(json["steps"][1]["program"], "ctest");
    assert_eq!(
        json["steps"][1]["args"],
        serde_json::json!([
            "--test-dir",
            format!("{root}/out/build/debug"),
            "-R",
            "foo",
            "--output-on-failure"
        ])
    );
    assert!(json["steps"][0].get("bootstrap").is_none());
    assert!(json["steps"][1].get("bootstrap").is_none());

    let text = serde_json::to_string(&json).expect("json text");
    assert!(
        !text.contains('\\'),
        "plan JSON paths should use slashes: {text}"
    );
}

#[test]
fn check_dry_run_json_matches_plan_json() {
    let temp = fixture_project();
    let project = temp.path().to_str().unwrap();

    let plan = qtflow_json(&["--project", project, "plan", "check", "foo", "--json"]);
    let dry_run = qtflow_json(&["--project", project, "check", "foo", "--dry-run", "--json"]);

    assert_eq!(dry_run, plan);
}

#[test]
fn build_config_name_adds_cmake_build_config_arg() {
    let temp = fixture_project();
    let project = temp.path().to_str().unwrap();

    let json = qtflow_json(&[
        "--project",
        project,
        "plan",
        "build",
        "foo",
        "--config-name",
        "Debug",
        "--json",
    ]);
    let root = json["projectRoot"].as_str().expect("project root");

    assert_eq!(
        json["steps"][0]["args"],
        serde_json::json!([
            "--build",
            format!("{root}/out/build/debug"),
            "--config",
            "Debug",
            "--target",
            "foo"
        ])
    );
}

#[test]
fn test_config_name_adds_ctest_config_arg() {
    let temp = fixture_project();
    let project = temp.path().to_str().unwrap();

    let json = qtflow_json(&[
        "--project",
        project,
        "plan",
        "test",
        "smoke",
        "--config-name",
        "Release",
        "--json",
    ]);
    let root = json["projectRoot"].as_str().expect("project root");

    assert_eq!(
        json["steps"][0]["args"],
        serde_json::json!([
            "--test-dir",
            format!("{root}/out/build/debug"),
            "-C",
            "Release",
            "-R",
            "smoke",
            "--output-on-failure"
        ])
    );
}

#[test]
fn check_config_name_adds_build_and_ctest_config_args() {
    let temp = fixture_project();
    let project = temp.path().to_str().unwrap();

    let json = qtflow_json(&[
        "--project",
        project,
        "plan",
        "check",
        "foo",
        "--config-name",
        "Debug",
        "--json",
    ]);
    let root = json["projectRoot"].as_str().expect("project root");

    assert_eq!(
        json["steps"][0]["args"],
        serde_json::json!([
            "--build",
            format!("{root}/out/build/debug"),
            "--config",
            "Debug",
            "--target",
            "foo"
        ])
    );
    assert_eq!(
        json["steps"][1]["args"],
        serde_json::json!([
            "--test-dir",
            format!("{root}/out/build/debug"),
            "-C",
            "Debug",
            "-R",
            "foo",
            "--output-on-failure"
        ])
    );
}

#[test]
fn profile_config_name_is_used_and_cli_overrides_it() {
    let temp = fixture_project_with_config(
        r#"
default_profile = "debug"

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
config_name = "Debug"
ctest_args = []

[msvc]
enabled = false
"#,
    );
    let project = temp.path().to_str().unwrap();

    let profile_json = qtflow_json(&["--project", project, "plan", "check", "foo", "--json"]);
    assert_eq!(profile_json["steps"][0]["args"][2], "--config");
    assert_eq!(profile_json["steps"][0]["args"][3], "Debug");
    assert_eq!(profile_json["steps"][1]["args"][2], "-C");
    assert_eq!(profile_json["steps"][1]["args"][3], "Debug");

    let cli_json = qtflow_json(&[
        "--project",
        project,
        "plan",
        "check",
        "foo",
        "--config-name",
        "Release",
        "--json",
    ]);
    assert_eq!(cli_json["steps"][0]["args"][3], "Release");
    assert_eq!(cli_json["steps"][1]["args"][3], "Release");
}

#[test]
fn configure_without_preset_or_generator_exits_2() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.20)\n",
    )
    .expect("write CMakeLists");
    std::fs::write(
        temp.path().join(".qtflow.toml"),
        r#"
default_profile = "custom"

[profiles.custom]
preset = ""
build_dir = "out/build/custom"
"#,
    )
    .expect("write config");

    Command::cargo_bin("qtflow")
        .expect("binary")
        .env_remove("QTFLOW_CONFIG")
        .env_remove("QTFLOW_PROFILE")
        .env_remove("QTFLOW_CMAKE")
        .env_remove("QTFLOW_CTEST")
        .env_remove("QTFLOW_NINJA")
        .env_remove("QTFLOW_QMAKE")
        .env_remove("QTFLOW_VSDEVCMD_BAT")
        .env_remove("VSDEVCMD_BAT")
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["plan", "configure"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "no CMake preset or generator configured for profile 'custom'",
        ))
        .stderr(predicate::str::contains(
            "set profiles.custom.preset or profiles.custom.generator, or pass --preset/--generator",
        ));
}

#[test]
fn configure_with_file_defined_generator_omitting_preset_plans_explicit_configure() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.20)\n",
    )
    .expect("write CMakeLists");
    std::fs::write(
        temp.path().join(".qtflow.toml"),
        r#"
default_profile = "debug"

[profiles.debug]
build_dir = "build"
generator = "Ninja"
configure_args = ["-DCMAKE_MAKE_PROGRAM=ninja"]
ctest_args = ["--output-on-failure"]

[msvc]
enabled = false
"#,
    )
    .expect("write config");

    let output = Command::cargo_bin("qtflow")
        .expect("binary")
        .env_remove("QTFLOW_CONFIG")
        .env_remove("QTFLOW_PROFILE")
        .env_remove("QTFLOW_CMAKE")
        .env_remove("QTFLOW_CTEST")
        .env_remove("QTFLOW_NINJA")
        .env_remove("QTFLOW_QMAKE")
        .env_remove("QTFLOW_VSDEVCMD_BAT")
        .env_remove("VSDEVCMD_BAT")
        .args(["--project", temp.path().to_str().unwrap()])
        .args(["plan", "configure", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).expect("valid JSON");
    let root = json["projectRoot"].as_str().expect("project root");
    assert_eq!(json["steps"][0]["program"], "cmake");
    assert_eq!(
        json["steps"][0]["args"],
        serde_json::json!([
            "-S",
            root,
            "-B",
            format!("{root}/build"),
            "-G",
            "Ninja",
            "-DCMAKE_MAKE_PROGRAM=ninja"
        ])
    );
}

#[test]
fn configure_cache_variables_are_sorted_before_configure_args_for_preset() {
    let temp = fixture_project_with_config(
        r#"
[tools]
ninja = "C:/tools/ninja.exe"

[msvc]
enabled = false

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
configure_args = ["-DCMAKE_PREFIX_PATH=override"]

[profiles.debug.cache_variables]
CMAKE_TOOLCHAIN_FILE = "x"
CMAKE_PREFIX_PATH = "y"
"#,
    );
    let project = temp.path().to_str().unwrap();

    let json = qtflow_json(&["--project", project, "plan", "configure", "--json"]);

    assert_eq!(
        json["steps"][0]["args"],
        serde_json::json!([
            "--preset",
            "Qt-Debug",
            "-DCMAKE_PREFIX_PATH=y",
            "-DCMAKE_TOOLCHAIN_FILE=x",
            "-DCMAKE_PREFIX_PATH=override"
        ])
    );
}

#[test]
fn configure_ninja_injects_make_program_after_cache_variables_without_preset() {
    let temp = fixture_project_with_config(
        r#"
[tools]
ninja = "C:/tools/ninja.exe"

[msvc]
enabled = false

[profiles.debug]
build_dir = "build"
generator = "Ninja"
configure_args = ["-DCMAKE_PREFIX_PATH=override"]

[profiles.debug.cache_variables]
CMAKE_TOOLCHAIN_FILE = "x"
CMAKE_PREFIX_PATH = "y"
"#,
    );
    let project = temp.path().to_str().unwrap();

    let json = qtflow_json(&["--project", project, "plan", "configure", "--json"]);
    let root = json["projectRoot"].as_str().expect("project root");

    assert_eq!(
        json["steps"][0]["args"],
        serde_json::json!([
            "-S",
            root,
            "-B",
            format!("{root}/build"),
            "-G",
            "Ninja",
            "-DCMAKE_PREFIX_PATH=y",
            "-DCMAKE_TOOLCHAIN_FILE=x",
            "-DCMAKE_MAKE_PROGRAM=C:/tools/ninja.exe",
            "-DCMAKE_PREFIX_PATH=override"
        ])
    );
}

#[test]
fn configure_ninja_does_not_duplicate_make_program_from_cache_variables_or_preset() {
    let non_preset = fixture_project_with_config(
        r#"
[tools]
ninja = "C:/tools/ninja.exe"

[msvc]
enabled = false

[profiles.debug]
build_dir = "build"
generator = "Ninja"

[profiles.debug.cache_variables]
CMAKE_MAKE_PROGRAM = "custom-ninja"
"#,
    );
    let project = non_preset.path().to_str().unwrap();
    let json = qtflow_json(&["--project", project, "plan", "configure", "--json"]);
    let args = json["steps"][0]["args"].as_array().expect("args");

    assert_eq!(
        args.iter()
            .filter(|arg| arg
                .as_str()
                .is_some_and(|arg| arg.contains("CMAKE_MAKE_PROGRAM")))
            .count(),
        1
    );
    assert!(args
        .iter()
        .any(|arg| arg == "-DCMAKE_MAKE_PROGRAM=custom-ninja"));

    let preset = fixture_project_with_config(
        r#"
[tools]
ninja = "C:/tools/ninja.exe"

[msvc]
enabled = false

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
"#,
    );
    let project = preset.path().to_str().unwrap();
    let json = qtflow_json(&["--project", project, "plan", "configure", "--json"]);

    assert_eq!(
        json["steps"][0]["args"],
        serde_json::json!(["--preset", "Qt-Debug"])
    );
}

#[test]
fn plan_steps_include_path_prepend_from_qt_bin_dir_and_profile() {
    let temp = fixture_project_with_config(
        r#"
[qt]
bin_dir = "qt/bin"

[msvc]
enabled = false

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
path_prepend = ["tools/bin", "C:/extra/bin"]
ctest_args = []
"#,
    );
    let project = temp.path().to_str().unwrap();

    let json = qtflow_json(&["--project", project, "plan", "check", "foo", "--json"]);
    let root = json["projectRoot"].as_str().expect("project root");
    let expected = serde_json::json!([
        format!("{root}/qt/bin"),
        format!("{root}/tools/bin"),
        "C:/extra/bin"
    ]);

    assert_eq!(json["steps"][0]["pathPrepend"], expected);
    assert_eq!(json["steps"][1]["pathPrepend"], expected);
}

#[test]
fn valid_config_does_not_emit_unknown_key_warning_and_empty_path_prepend_is_absent() {
    let temp = fixture_project();
    let project = temp.path().to_str().unwrap();

    let output = Command::cargo_bin("qtflow")
        .expect("binary")
        .env_remove("QTFLOW_CONFIG")
        .env_remove("QTFLOW_PROFILE")
        .env_remove("QTFLOW_CMAKE")
        .env_remove("QTFLOW_CTEST")
        .env_remove("QTFLOW_NINJA")
        .env_remove("QTFLOW_QMAKE")
        .env_remove("QTFLOW_VSDEVCMD_BAT")
        .env_remove("VSDEVCMD_BAT")
        .args(["--project", project, "plan", "check", "foo", "--json"])
        .assert()
        .success()
        .stderr(predicate::str::contains("unknown key").not())
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).expect("valid JSON");
    assert!(json["steps"][0].get("pathPrepend").is_none());
}

#[test]
fn unknown_config_key_warns_to_stderr_and_quiet_suppresses_it() {
    let temp = fixture_project_with_config(
        r#"
[msvc]
enabled = false

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
cmake_args = ["-DOPT=ON"]
"#,
    );
    let project = temp.path().to_str().unwrap();

    Command::cargo_bin("qtflow")
        .expect("binary")
        .env_remove("QTFLOW_CONFIG")
        .env_remove("QTFLOW_PROFILE")
        .env_remove("QTFLOW_CMAKE")
        .env_remove("QTFLOW_CTEST")
        .env_remove("QTFLOW_NINJA")
        .env_remove("QTFLOW_QMAKE")
        .env_remove("QTFLOW_VSDEVCMD_BAT")
        .env_remove("VSDEVCMD_BAT")
        .args(["--project", project, "plan", "configure"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "warning: unknown key 'cmake_args' in [profiles.debug] (ignored)",
        ));

    Command::cargo_bin("qtflow")
        .expect("binary")
        .env_remove("QTFLOW_CONFIG")
        .env_remove("QTFLOW_PROFILE")
        .env_remove("QTFLOW_CMAKE")
        .env_remove("QTFLOW_CTEST")
        .env_remove("QTFLOW_NINJA")
        .env_remove("QTFLOW_QMAKE")
        .env_remove("QTFLOW_VSDEVCMD_BAT")
        .env_remove("VSDEVCMD_BAT")
        .args(["--project", project, "--quiet", "plan", "configure"])
        .assert()
        .success()
        .stderr(predicate::str::contains("unknown key").not());
}
