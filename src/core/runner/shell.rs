use std::path::Path;

use crate::core::path::path_to_slash;
use crate::core::plan::{CommandStep, EnvironmentBootstrap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub raw_arg: Option<String>,
}

pub fn quote_for_cmd(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| quote_arg_for_cmd(part))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn render_command_display(step: &CommandStep) -> String {
    match &step.bootstrap {
        Some(EnvironmentBootstrap::Msvc {
            vsdevcmd,
            arch,
            host_arch,
        }) => {
            let inner = msvc_inner_command(vsdevcmd, arch, host_arch.as_deref(), step);
            format!("cmd.exe /d /s /c \"{inner}\"")
        }
        None => {
            let parts = command_parts(step);
            quote_for_cmd(&parts)
        }
    }
}

pub fn command_for_step(step: &CommandStep) -> CommandSpec {
    if cfg!(windows) {
        if let Some(EnvironmentBootstrap::Msvc {
            vsdevcmd,
            arch,
            host_arch,
        }) = &step.bootstrap
        {
            return CommandSpec {
                program: "cmd".to_string(),
                args: vec!["/d".to_string(), "/s".to_string(), "/c".to_string()],
                raw_arg: Some(msvc_inner_command(
                    vsdevcmd,
                    arch,
                    host_arch.as_deref(),
                    step,
                )),
            };
        }
    }

    CommandSpec {
        program: step.program.clone(),
        args: step.args.clone(),
        raw_arg: None,
    }
}

pub fn run_vswhere(vswhere: &Path) -> Option<String> {
    let program = if vswhere.components().count() > 1 {
        vswhere.to_path_buf()
    } else {
        which::which(vswhere).ok()?
    };

    let output = std::process::Command::new(program)
        .args(["-latest", "-products", "*", "-property", "installationPath"])
        .output()
        .ok()?;

    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn msvc_inner_command(
    vsdevcmd: &Path,
    arch: &str,
    host_arch: Option<&str>,
    step: &CommandStep,
) -> String {
    let mut bootstrap_parts = vec![
        "call".to_string(),
        path_to_slash(vsdevcmd),
        format!("-arch={arch}"),
    ];
    if let Some(host_arch) = host_arch {
        bootstrap_parts.push(format!("-host_arch={host_arch}"));
    }

    let bootstrap_refs = bootstrap_parts
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let command_parts = command_parts(step);
    let path_prepend = msvc_path_prepend_command(&step.path_prepend);

    match path_prepend {
        Some(path_prepend) => format!(
            "{} && {} && {}",
            quote_for_cmd(&bootstrap_refs),
            path_prepend,
            quote_for_cmd(&command_parts)
        ),
        None => format!(
            "{} && {}",
            quote_for_cmd(&bootstrap_refs),
            quote_for_cmd(&command_parts)
        ),
    }
}

fn msvc_path_prepend_command(path_prepend: &[String]) -> Option<String> {
    let joined = path_prepend
        .iter()
        .filter(|entry| !entry.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(";");
    (!joined.is_empty()).then(|| format!("set \"PATH={};%PATH%\"", escape_set_value(&joined)))
}

fn escape_set_value(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\"\""),
            '&' | '|' | '<' | '>' | '^' => {
                escaped.push('^');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn command_parts(step: &CommandStep) -> Vec<&str> {
    std::iter::once(step.program.as_str())
        .chain(step.args.iter().map(String::as_str))
        .collect()
}

fn quote_arg_for_cmd(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }

    let needs_quotes = arg.chars().any(|ch| {
        ch.is_whitespace()
            || matches!(
                ch,
                '"' | '&' | '|' | '<' | '>' | '^' | '(' | ')' | '%' | '!'
            )
    });
    let mut escaped = String::new();
    for ch in arg.chars() {
        match ch {
            '"' => escaped.push_str("\"\""),
            '&' | '|' | '<' | '>' | '^' | '(' | ')' | '%' | '!' => {
                escaped.push('^');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }

    if needs_quotes {
        format!("\"{escaped}\"")
    } else {
        escaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plan::CommandStep;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn step(bootstrap: Option<EnvironmentBootstrap>) -> CommandStep {
        CommandStep {
            label: "build".to_string(),
            cwd: PathBuf::from("C:/repo"),
            program: "cmake".to_string(),
            args: vec![
                "--build".to_string(),
                "C:/repo/out/build/debug".to_string(),
                "--target".to_string(),
                "foo".to_string(),
            ],
            env: BTreeMap::new(),
            path_prepend: Vec::new(),
            bootstrap,
        }
    }

    #[test]
    fn quote_for_cmd_handles_path_with_spaces() {
        assert_eq!(
            quote_for_cmd(&["C:/Program Files/CMake/bin/cmake.exe", "--version"]),
            "\"C:/Program Files/CMake/bin/cmake.exe\" --version"
        );
    }

    #[test]
    fn quote_for_cmd_handles_embedded_double_quote() {
        assert_eq!(
            quote_for_cmd(&["cmake", "-DNAME=a\"b"]),
            "cmake \"-DNAME=a\"\"b\""
        );
    }

    #[test]
    fn quote_for_cmd_handles_double_ampersand_as_data() {
        assert_eq!(
            quote_for_cmd(&["tool", "left&&right"]),
            "tool \"left^&^&right\""
        );
    }

    #[test]
    fn quote_for_cmd_handles_caret() {
        assert_eq!(quote_for_cmd(&["tool", "a^b"]), "tool \"a^^b\"");
    }

    #[test]
    fn quote_for_cmd_handles_empty_arg() {
        assert_eq!(quote_for_cmd(&["tool", ""]), "tool \"\"");
    }

    #[test]
    fn quote_for_cmd_handles_cmd_metachars() {
        assert_eq!(
            quote_for_cmd(&["tool", "(a|b)<c>d"]),
            "tool \"^(a^|b^)^<c^>d\""
        );
    }

    #[test]
    fn render_command_display_for_direct_step_uses_cmd_quoting() {
        let step = CommandStep {
            label: "custom".to_string(),
            cwd: PathBuf::from("C:/repo"),
            program: "C:/Program Files/tool.exe".to_string(),
            args: vec!["left&&right".to_string(), "".to_string()],
            env: BTreeMap::new(),
            path_prepend: Vec::new(),
            bootstrap: None,
        };

        assert_eq!(
            render_command_display(&step),
            "\"C:/Program Files/tool.exe\" \"left^&^&right\" \"\""
        );
    }

    #[test]
    fn render_command_display_for_bootstrap_matches_cmd_shape() {
        let display = render_command_display(&step(Some(EnvironmentBootstrap::Msvc {
            vsdevcmd: PathBuf::from("C:/Program Files/Microsoft Visual Studio/2022/Community/Common7/Tools/VsDevCmd.bat"),
            arch: "x64".to_string(),
            host_arch: Some("x64".to_string()),
        })));

        assert_eq!(
            display,
            "cmd.exe /d /s /c \"call \"C:/Program Files/Microsoft Visual Studio/2022/Community/Common7/Tools/VsDevCmd.bat\" -arch=x64 -host_arch=x64 && cmake --build C:/repo/out/build/debug --target foo\""
        );
    }

    #[test]
    fn render_command_display_for_bootstrap_includes_path_prepend_inside_cmd_session() {
        let mut step = step(Some(EnvironmentBootstrap::Msvc {
            vsdevcmd: PathBuf::from("C:/VS/Common7/Tools/VsDevCmd.bat"),
            arch: "x64".to_string(),
            host_arch: None,
        }));
        step.path_prepend = vec!["C:/Qt/bin".to_string(), "C:/tools/bin".to_string()];

        assert_eq!(
            render_command_display(&step),
            "cmd.exe /d /s /c \"call C:/VS/Common7/Tools/VsDevCmd.bat -arch=x64 && set \"PATH=C:/Qt/bin;C:/tools/bin;%PATH%\" && cmake --build C:/repo/out/build/debug --target foo\""
        );
    }

    #[test]
    fn command_for_step_direct_is_program_and_args() {
        let step = step(None);

        assert_eq!(
            command_for_step(&step),
            CommandSpec {
                program: "cmake".to_string(),
                args: vec![
                    "--build".to_string(),
                    "C:/repo/out/build/debug".to_string(),
                    "--target".to_string(),
                    "foo".to_string()
                ],
                raw_arg: None
            }
        );
    }

    #[cfg(windows)]
    #[test]
    fn command_for_bootstrap_uses_raw_cmd_argument_on_windows() {
        let step = step(Some(EnvironmentBootstrap::Msvc {
            vsdevcmd: PathBuf::from("C:/Program Files/VS/Common7/Tools/VsDevCmd.bat"),
            arch: "x64".to_string(),
            host_arch: None,
        }));

        assert_eq!(
            command_for_step(&step),
            CommandSpec {
                program: "cmd".to_string(),
                args: vec!["/d".to_string(), "/s".to_string(), "/c".to_string()],
                raw_arg: Some(
                    "call \"C:/Program Files/VS/Common7/Tools/VsDevCmd.bat\" -arch=x64 && cmake --build C:/repo/out/build/debug --target foo"
                        .to_string()
                )
            }
        );
    }

    #[test]
    fn render_command_display_equals_dry_run_step_text_command() {
        let step = step(Some(EnvironmentBootstrap::Msvc {
            vsdevcmd: PathBuf::from("C:/VS/Common7/Tools/VsDevCmd.bat"),
            arch: "x64".to_string(),
            host_arch: None,
        }));
        let dry_run_line = format!(
            "{}: {}  [msvc: {} arch={}]",
            step.label,
            render_command_display(&step),
            path_to_slash(Path::new("C:/VS/Common7/Tools/VsDevCmd.bat")),
            "x64"
        );

        assert_eq!(
            dry_run_line,
            "build: cmd.exe /d /s /c \"call C:/VS/Common7/Tools/VsDevCmd.bat -arch=x64 && cmake --build C:/repo/out/build/debug --target foo\"  [msvc: C:/VS/Common7/Tools/VsDevCmd.bat arch=x64]"
        );
    }
}
