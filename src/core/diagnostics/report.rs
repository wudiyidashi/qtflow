use serde::Serialize;

use super::Finding;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    pub exit_code: i32,
    pub diagnostics: Vec<Finding>,
}

pub fn render_json(report: &DiagnosticReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

pub fn render_text(error: &str, findings: &[Finding]) -> String {
    let mut output = String::new();
    output.push_str("error: ");
    output.push_str(error);
    output.push('\n');

    for finding in findings {
        output.push('\n');
        output.push_str("diagnostic: ");
        output.push_str(&finding.title);
        output.push('\n');
        output.push_str("evidence:\n");
        for line in &finding.evidence {
            output.push_str("  ");
            output.push_str(line);
            output.push('\n');
        }
        output.push_str("why:\n");
        output.push_str("  ");
        output.push_str(&finding.explanation);
        output.push('\n');
        output.push_str("fix:\n");
        for command in &finding.suggested_commands {
            output.push_str("  ");
            output.push_str(command);
            output.push('\n');
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diagnostics::Severity;

    fn finding() -> Finding {
        Finding {
            code: "msvc.missing_standard_headers".to_string(),
            severity: Severity::Error,
            title: "MSVC standard headers are unavailable".to_string(),
            evidence: vec!["fatal error C1083: cannot open include file: 'string'".to_string()],
            explanation:
                "The build likely ran without Visual Studio developer environment variables."
                    .to_string(),
            suggested_commands: vec!["qtflow doctor".to_string()],
        }
    }

    #[test]
    fn json_uses_contract_shape_and_camel_case() {
        let json = render_json(&DiagnosticReport {
            exit_code: 1,
            diagnostics: vec![finding()],
        })
        .expect("json");
        let value: serde_json::Value = serde_json::from_str(&json).expect("json value");

        assert_eq!(value["exitCode"], 1);
        assert_eq!(
            value["diagnostics"][0]["suggestedCommands"],
            serde_json::json!(["qtflow doctor"])
        );
    }

    #[test]
    fn text_uses_contract_block_labels() {
        let text = render_text("build failed", &[finding()]);

        assert!(text.contains("diagnostic: MSVC standard headers are unavailable\n"));
        assert!(text.contains("evidence:\n"));
        assert!(text.contains("why:\n"));
        assert!(text.contains("fix:\n"));
    }
}
