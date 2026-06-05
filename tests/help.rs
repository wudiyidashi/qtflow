use assert_cmd::Command;

#[test]
fn top_level_help_lists_commands_with_content() {
    let output = Command::cargo_bin("qtflow")
        .expect("binary")
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8(output).expect("help is utf8");

    assert!(!help.trim().is_empty());
    for command in [
        "doctor",
        "configure",
        "build",
        "test",
        "check",
        "plan",
        "init",
    ] {
        assert!(help.contains(command), "missing {command} in help:\n{help}");
    }
}

#[test]
fn check_help_mentions_ctest() {
    let output = Command::cargo_bin("qtflow")
        .expect("binary")
        .args(["check", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8(output).expect("help is utf8");

    assert!(
        help.contains("CTest"),
        "check help should mention CTest:\n{help}"
    );
}
