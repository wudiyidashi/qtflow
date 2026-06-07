use serde::Serialize;

use crate::core::project::ProjectKind;

pub mod report;
pub mod rules;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Configure,
    Build,
    Test,
    Deploy,
}

impl CommandKind {
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "configure" => Some(Self::Configure),
            "build" => Some(Self::Build),
            "test" => Some(Self::Test),
            "deploy" => Some(Self::Deploy),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Unix,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticContext<'a> {
    pub exit_code: i32,
    pub command_kind: CommandKind,
    pub project_kind: ProjectKind,
    pub combined_log: &'a str,
    pub platform: Platform,
    pub bootstrap_used: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Rule {
    pub code: &'static str,
    pub severity: Severity,
    pub patterns: &'static [&'static str],
    pub applies_to: &'static [CommandKind],
    pub project_kinds: &'static [ProjectKind],
    pub title: &'static str,
    pub explanation: &'static str,
    pub suggested: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    pub title: String,
    pub evidence: Vec<String>,
    pub explanation: String,
    pub suggested_commands: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Engine {
    max_log_bytes: usize,
}

impl Engine {
    pub fn new(max_log_bytes: usize) -> Self {
        Self { max_log_bytes }
    }

    pub fn analyze(&self, ctx: &DiagnosticContext<'_>) -> Vec<Finding> {
        let log = truncate_to_max_bytes(ctx.combined_log, self.max_log_bytes);
        rules::RULES
            .iter()
            .filter(|rule| rule.applies_to.contains(&ctx.command_kind))
            .filter(|rule| rule.project_kinds.contains(&ctx.project_kind))
            .filter_map(|rule| finding_for_rule(rule, log))
            .collect()
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            max_log_bytes: 200_000,
        }
    }
}

pub fn exit_code_override(findings: &[Finding], bootstrap_used: bool) -> Option<i32> {
    if findings
        .iter()
        .any(|finding| finding.code == "msvc.vsdevcmd_not_found")
    {
        return Some(6);
    }

    if !bootstrap_used
        && findings
            .iter()
            .any(|finding| finding.code == "msvc.missing_standard_headers")
    {
        return Some(6);
    }

    if findings.iter().any(|finding| {
        matches!(
            finding.code.as_str(),
            "tool.cmake_not_found"
                | "tool.ctest_not_found"
                | "qmake.not_found"
                | "make.tool_not_found"
                | "deploy.tool_not_found"
        )
    }) {
        return Some(3);
    }

    None
}

fn finding_for_rule(rule: &Rule, log: &str) -> Option<Finding> {
    let evidence = matching_lines(log, rule.patterns);
    (!evidence.is_empty()).then(|| Finding {
        code: rule.code.to_string(),
        severity: rule.severity,
        title: rule.title.to_string(),
        evidence,
        explanation: rule.explanation.to_string(),
        suggested_commands: rule
            .suggested
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    })
}

fn matching_lines(log: &str, patterns: &[&str]) -> Vec<String> {
    let mut evidence = Vec::new();
    for line in log.lines() {
        if patterns.iter().any(|pattern| line.contains(pattern)) {
            let line = line.trim().to_string();
            if !line.is_empty() && !evidence.contains(&line) {
                evidence.push(line);
            }
        }
    }
    evidence
}

fn truncate_to_max_bytes(log: &str, max_bytes: usize) -> &str {
    if log.len() <= max_bytes {
        return log;
    }
    if max_bytes == 0 {
        return "";
    }

    let mut start = log.len() - max_bytes;
    while !log.is_char_boundary(start) {
        start += 1;
    }
    &log[start..]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(command_kind: CommandKind, log: &str) -> DiagnosticContext<'_> {
        DiagnosticContext {
            exit_code: 1,
            command_kind,
            project_kind: ProjectKind::Cmake,
            combined_log: log,
            platform: Platform::Windows,
            bootstrap_used: false,
        }
    }

    fn project_ctx(
        project_kind: ProjectKind,
        command_kind: CommandKind,
        log: &str,
    ) -> DiagnosticContext<'_> {
        DiagnosticContext {
            project_kind,
            ..ctx(command_kind, log)
        }
    }

    #[test]
    fn required_rules_match_expected_codes_and_evidence() {
        let cases = [
            (
                CommandKind::Build,
                "fatal error C1083: cannot open include file: 'string'\n",
                "msvc.missing_standard_headers",
            ),
            (
                CommandKind::Build,
                "VsDevCmd.bat was not found\n",
                "msvc.vsdevcmd_not_found",
            ),
            (
                CommandKind::Build,
                "Error: could not load cache\n",
                "cmake.build_dir_missing",
            ),
            (
                CommandKind::Configure,
                "CMake Error: No such preset\n",
                "cmake.preset_missing",
            ),
            (
                CommandKind::Build,
                "No such file or directory: cmake\n",
                "tool.cmake_not_found",
            ),
            (
                CommandKind::Test,
                "No such file or directory: ctest\n",
                "tool.ctest_not_found",
            ),
            (
                CommandKind::Configure,
                "qmake: command not found\n",
                "qmake.not_found",
            ),
            (
                CommandKind::Configure,
                "Could not find qmake configuration file win32-msvc\n",
                "qmake.spec_missing",
            ),
            (
                CommandKind::Build,
                "'nmake' is not recognized as an internal or external command\n",
                "make.tool_not_found",
            ),
            (
                CommandKind::Deploy,
                "'windeployqt' is not recognized as an internal or external command\n",
                "deploy.tool_not_found",
            ),
        ];

        for (kind, log, code) in cases {
            let project_kind = if code.starts_with("qmake.") || code.starts_with("make.") {
                ProjectKind::Qmake
            } else {
                ProjectKind::Cmake
            };
            let findings = Engine::default().analyze(&project_ctx(project_kind, kind, log));

            let finding = findings
                .iter()
                .find(|finding| finding.code == code)
                .unwrap_or_else(|| panic!("expected {code} in {findings:?}"));
            assert!(
                finding
                    .evidence
                    .iter()
                    .any(|line| log.trim().contains(line)),
                "evidence should capture matching line for {code}: {finding:?}"
            );
        }
    }

    #[test]
    fn applies_to_filters_rules_by_command_kind() {
        let findings = Engine::default().analyze(&ctx(
            CommandKind::Test,
            "fatal error C1083: cannot open include file: 'string'\n",
        ));

        assert!(findings
            .iter()
            .all(|finding| finding.code != "msvc.missing_standard_headers"));
    }

    #[test]
    fn project_kind_filters_cmake_and_qmake_specific_rules() {
        let qmake_findings = Engine::default().analyze(&project_ctx(
            ProjectKind::Qmake,
            CommandKind::Build,
            "Error: could not load cache\n",
        ));
        assert!(qmake_findings
            .iter()
            .all(|finding| finding.code != "cmake.build_dir_missing"));

        let cmake_findings = Engine::default().analyze(&project_ctx(
            ProjectKind::Cmake,
            CommandKind::Configure,
            "Could not find qmake configuration file win32-msvc\n",
        ));
        assert!(cmake_findings
            .iter()
            .all(|finding| finding.code != "qmake.spec_missing"));
    }

    #[test]
    fn exit_override_maps_known_setup_and_tool_failures() {
        let vsdevcmd = Finding {
            code: "msvc.vsdevcmd_not_found".to_string(),
            severity: Severity::Error,
            title: String::new(),
            evidence: Vec::new(),
            explanation: String::new(),
            suggested_commands: Vec::new(),
        };
        let headers = Finding {
            code: "msvc.missing_standard_headers".to_string(),
            ..vsdevcmd.clone()
        };
        let cmake = Finding {
            code: "tool.cmake_not_found".to_string(),
            ..vsdevcmd.clone()
        };
        let qmake = Finding {
            code: "qmake.not_found".to_string(),
            ..vsdevcmd.clone()
        };
        let deploy = Finding {
            code: "deploy.tool_not_found".to_string(),
            ..vsdevcmd.clone()
        };

        assert_eq!(exit_code_override(&[vsdevcmd], true), Some(6));
        assert_eq!(
            exit_code_override(std::slice::from_ref(&headers), false),
            Some(6)
        );
        assert_eq!(exit_code_override(&[headers], true), None);
        assert_eq!(exit_code_override(&[cmake], false), Some(3));
        assert_eq!(exit_code_override(&[qmake], false), Some(3));
        assert_eq!(exit_code_override(&[deploy], false), Some(3));
        assert_eq!(exit_code_override(&[], false), None);
    }
}
