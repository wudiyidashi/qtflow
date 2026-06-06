use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::process::{ChildStderr, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::core::plan::CommandPlan;
use crate::core::runner::shell::{command_for_step, render_command_display};
use crate::error::QtflowError;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub mod shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOptions {
    pub quiet: bool,
    pub verbose: bool,
    pub max_log_bytes: usize,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            quiet: false,
            verbose: false,
            max_log_bytes: 200_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub steps_run: usize,
    pub last_exit_code: i32,
    pub failure: Option<RunFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFailure {
    pub step_label: String,
    pub combined_log: String,
    pub bootstrap_used: bool,
}

pub fn execute_plan(plan: &CommandPlan, opts: &RunOptions) -> Result<RunOutcome, QtflowError> {
    let mut outcome = RunOutcome {
        steps_run: 0,
        last_exit_code: 0,
        failure: None,
    };

    for step in &plan.steps {
        if opts.verbose && !opts.quiet {
            eprintln!("+ {}", render_command_display(step));
            if !step.path_prepend.is_empty() {
                eprintln!("  path-prepend: {}", step.path_prepend.join(", "));
            }
        }

        let spec = command_for_step(step);
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .current_dir(&step.cwd)
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(&step.env);
        apply_path_prepend(&mut command, &step.path_prepend, &step.env);
        apply_raw_arg(&mut command, spec.raw_arg.as_deref());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(source) => {
                let mut captured = CappedLog::new(opts.max_log_bytes);
                captured.append(
                    format!(
                        "failed to spawn command '{}': {source}\nNo such file or directory\n",
                        spec.program
                    )
                    .as_bytes(),
                );
                outcome.steps_run += 1;
                outcome.last_exit_code = 1;
                outcome.failure = Some(RunFailure {
                    step_label: step.label.clone(),
                    combined_log: captured.into_string(),
                    bootstrap_used: step.bootstrap.is_some(),
                });
                return Ok(outcome);
            }
        };

        let captured = Arc::new(Mutex::new(CappedLog::new(opts.max_log_bytes)));
        let stdout_thread = child
            .stdout
            .take()
            .map(|stdout| tee_stdout(stdout, Arc::clone(&captured), !opts.quiet));
        let stderr_thread = child
            .stderr
            .take()
            .map(|stderr| tee_stderr(stderr, Arc::clone(&captured), !opts.quiet));

        let status = child.wait().map_err(|source| QtflowError::CommandSpawn {
            program: spec.program.clone(),
            source,
        })?;
        join_reader(stdout_thread)?;
        join_reader(stderr_thread)?;
        let exit_code = status.code().unwrap_or(1);

        outcome.steps_run += 1;
        outcome.last_exit_code = exit_code;

        if exit_code != 0 {
            outcome.failure = Some(RunFailure {
                step_label: step.label.clone(),
                combined_log: captured
                    .lock()
                    .map_err(|_| QtflowError::ConfigOrArg("runner log lock poisoned".to_string()))?
                    .as_string(),
                bootstrap_used: step.bootstrap.is_some(),
            });
            return Ok(outcome);
        }
    }

    Ok(outcome)
}

fn tee_stdout(
    stdout: ChildStdout,
    captured: Arc<Mutex<CappedLog>>,
    stream: bool,
) -> JoinHandle<io::Result<()>> {
    tee_pipe(stdout, captured, stream, false)
}

fn tee_stderr(
    stderr: ChildStderr,
    captured: Arc<Mutex<CappedLog>>,
    stream: bool,
) -> JoinHandle<io::Result<()>> {
    tee_pipe(stderr, captured, stream, true)
}

fn tee_pipe<R>(
    mut pipe: R,
    captured: Arc<Mutex<CappedLog>>,
    stream: bool,
    stderr: bool,
) -> JoinHandle<io::Result<()>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            let read = pipe.read(&mut buffer)?;
            if read == 0 {
                return Ok(());
            }
            let chunk = &buffer[..read];
            if stream {
                if stderr {
                    io::stderr().write_all(chunk)?;
                    io::stderr().flush()?;
                } else {
                    io::stdout().write_all(chunk)?;
                    io::stdout().flush()?;
                }
            }
            captured
                .lock()
                .map_err(|_| io::Error::other("runner log lock poisoned"))?
                .append(chunk);
        }
    })
}

fn join_reader(handle: Option<JoinHandle<io::Result<()>>>) -> Result<(), QtflowError> {
    match handle {
        Some(handle) => handle
            .join()
            .map_err(|_| QtflowError::ConfigOrArg("runner output thread panicked".to_string()))?
            .map_err(|source| QtflowError::CommandSpawn {
                program: "<runner-output>".to_string(),
                source,
            }),
        None => Ok(()),
    }
}

fn apply_path_prepend(
    command: &mut Command,
    path_prepend: &[String],
    step_env: &std::collections::BTreeMap<String, String>,
) {
    let path_key = path_env_key(step_env);
    let existing = step_env
        .get(&path_key)
        .map(OsString::from)
        .or_else(|| std::env::var_os(&path_key));
    if let Some(path) = prepend_path_entries_os(path_prepend, existing) {
        command.env(path_key, path);
    }
}

fn prepend_path_entries_os(
    path_prepend: &[String],
    existing_path: Option<OsString>,
) -> Option<OsString> {
    let prefix = joined_path_prepend(path_prepend, PATH_SEPARATOR)?;
    let mut value = OsString::from(prefix);
    if let Some(existing_path) = existing_path.filter(|path| !path.is_empty()) {
        value.push(PATH_SEPARATOR_STR);
        value.push(existing_path);
    }
    Some(value)
}

#[cfg(test)]
pub(crate) fn prepend_path_entries(
    path_prepend: &[String],
    existing_path: Option<&str>,
    separator: char,
) -> Option<String> {
    let mut value = joined_path_prepend(path_prepend, separator)?;
    if let Some(existing_path) = existing_path.filter(|path| !path.is_empty()) {
        value.push(separator);
        value.push_str(existing_path);
    }
    Some(value)
}

fn joined_path_prepend(path_prepend: &[String], separator: char) -> Option<String> {
    let entries = path_prepend
        .iter()
        .filter(|entry| !entry.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    (!entries.is_empty()).then(|| entries.join(&separator.to_string()))
}

fn path_env_key(step_env: &std::collections::BTreeMap<String, String>) -> String {
    step_env
        .keys()
        .find(|key| key.eq_ignore_ascii_case("PATH"))
        .cloned()
        .unwrap_or_else(|| "PATH".to_string())
}

#[cfg(windows)]
const PATH_SEPARATOR: char = ';';
#[cfg(not(windows))]
const PATH_SEPARATOR: char = ':';

#[cfg(windows)]
const PATH_SEPARATOR_STR: &str = ";";
#[cfg(not(windows))]
const PATH_SEPARATOR_STR: &str = ":";

#[derive(Debug, Clone)]
struct CappedLog {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl CappedLog {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
        }
    }

    fn append(&mut self, chunk: &[u8]) {
        if self.max_bytes == 0 || chunk.is_empty() {
            return;
        }

        if chunk.len() >= self.max_bytes {
            self.bytes.clear();
            self.bytes
                .extend_from_slice(&chunk[chunk.len() - self.max_bytes..]);
            return;
        }

        let overflow = self.bytes.len() + chunk.len();
        if overflow > self.max_bytes {
            self.bytes.drain(0..(overflow - self.max_bytes));
        }
        self.bytes.extend_from_slice(chunk);
    }

    fn as_string(&self) -> String {
        String::from_utf8_lossy(&self.bytes).to_string()
    }

    fn into_string(self) -> String {
        String::from_utf8_lossy(&self.bytes).to_string()
    }
}

#[cfg(windows)]
fn apply_raw_arg(command: &mut Command, raw_arg: Option<&str>) {
    if let Some(raw_arg) = raw_arg {
        command.raw_arg(raw_arg);
    }
}

#[cfg(not(windows))]
fn apply_raw_arg(_command: &mut Command, _raw_arg: Option<&str>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plan::CommandStep;
    use std::collections::BTreeMap;

    fn plan_for_shell(exit_code: i32) -> CommandPlan {
        #[cfg(windows)]
        let (program, args) = (
            "cmd".to_string(),
            vec![
                "/d".to_string(),
                "/s".to_string(),
                "/c".to_string(),
                format!("exit {exit_code}"),
            ],
        );

        #[cfg(not(windows))]
        let (program, args) = (
            "sh".to_string(),
            vec!["-c".to_string(), format!("exit {exit_code}")],
        );

        CommandPlan {
            project_root: std::env::current_dir().expect("cwd"),
            profile: "debug".to_string(),
            notes: Vec::new(),
            steps: vec![CommandStep {
                label: "test".to_string(),
                cwd: std::env::current_dir().expect("cwd"),
                program,
                args,
                env: BTreeMap::new(),
                path_prepend: Vec::new(),
                bootstrap: None,
            }],
        }
    }

    #[test]
    fn execute_plan_reports_success_outcome() {
        let outcome =
            execute_plan(&plan_for_shell(0), &RunOptions::default()).expect("execute plan");

        assert_eq!(
            outcome,
            RunOutcome {
                steps_run: 1,
                last_exit_code: 0,
                failure: None
            }
        );
    }

    #[test]
    fn execute_plan_stops_on_first_non_zero_step() {
        let outcome =
            execute_plan(&plan_for_shell(17), &RunOptions::default()).expect("execute plan");

        assert_eq!((outcome.steps_run, outcome.last_exit_code), (1, 17));
        assert!(outcome
            .failure
            .as_ref()
            .expect("failure")
            .combined_log
            .is_empty());
    }

    #[test]
    fn execute_plan_merges_step_env() {
        let cwd = std::env::current_dir().expect("cwd");
        #[cfg(windows)]
        let (program, args) = (
            "cmd".to_string(),
            vec![
                "/d".to_string(),
                "/s".to_string(),
                "/c".to_string(),
                "if \"%QTFLOW_RUNNER_TEST_ENV%\"==\"ok\" (exit 0) else (exit 9)".to_string(),
            ],
        );
        #[cfg(not(windows))]
        let (program, args) = (
            "sh".to_string(),
            vec![
                "-c".to_string(),
                "[ \"$QTFLOW_RUNNER_TEST_ENV\" = ok ]".to_string(),
            ],
        );
        let plan = CommandPlan {
            project_root: cwd.clone(),
            profile: "debug".to_string(),
            notes: Vec::new(),
            steps: vec![CommandStep {
                label: "env".to_string(),
                cwd,
                program,
                args,
                env: BTreeMap::from([("QTFLOW_RUNNER_TEST_ENV".to_string(), "ok".to_string())]),
                path_prepend: Vec::new(),
                bootstrap: None,
            }],
        };

        let outcome = execute_plan(&plan, &RunOptions::default()).expect("execute plan");

        assert_eq!(outcome.last_exit_code, 0);
    }

    #[test]
    fn execute_plan_captures_failing_stderr() {
        let cwd = std::env::current_dir().expect("cwd");
        #[cfg(windows)]
        let (program, args) = (
            "cmd".to_string(),
            vec![
                "/d".to_string(),
                "/s".to_string(),
                "/c".to_string(),
                "echo Error: could not load cache 1>&2 && exit 9".to_string(),
            ],
        );
        #[cfg(not(windows))]
        let (program, args) = (
            "sh".to_string(),
            vec![
                "-c".to_string(),
                "echo 'Error: could not load cache' >&2; exit 9".to_string(),
            ],
        );
        let plan = CommandPlan {
            project_root: cwd.clone(),
            profile: "debug".to_string(),
            notes: Vec::new(),
            steps: vec![CommandStep {
                label: "build".to_string(),
                cwd,
                program,
                args,
                env: BTreeMap::new(),
                path_prepend: Vec::new(),
                bootstrap: None,
            }],
        };

        let outcome = execute_plan(
            &plan,
            &RunOptions {
                quiet: true,
                ..RunOptions::default()
            },
        )
        .expect("execute plan");

        assert_eq!(outcome.last_exit_code, 9);
        assert!(outcome
            .failure
            .as_ref()
            .expect("failure")
            .combined_log
            .contains("Error: could not load cache"));
    }

    #[test]
    fn captured_failure_log_can_feed_diagnostics_engine() {
        use crate::core::diagnostics::{CommandKind, DiagnosticContext, Engine, Platform};
        use crate::core::project::ProjectKind;

        let cwd = std::env::current_dir().expect("cwd");
        #[cfg(windows)]
        let (program, args) = (
            "cmd".to_string(),
            vec![
                "/d".to_string(),
                "/s".to_string(),
                "/c".to_string(),
                "echo Error: could not load cache 1>&2 && exit 9".to_string(),
            ],
        );
        #[cfg(not(windows))]
        let (program, args) = (
            "sh".to_string(),
            vec![
                "-c".to_string(),
                "echo 'Error: could not load cache' >&2; exit 9".to_string(),
            ],
        );
        let plan = CommandPlan {
            project_root: cwd.clone(),
            profile: "debug".to_string(),
            notes: Vec::new(),
            steps: vec![CommandStep {
                label: "build".to_string(),
                cwd,
                program,
                args,
                env: BTreeMap::new(),
                path_prepend: Vec::new(),
                bootstrap: None,
            }],
        };

        let outcome = execute_plan(
            &plan,
            &RunOptions {
                quiet: true,
                ..RunOptions::default()
            },
        )
        .expect("execute plan");
        let failure = outcome.failure.as_ref().expect("failure");
        let findings = Engine::default().analyze(&DiagnosticContext {
            exit_code: outcome.last_exit_code,
            command_kind: CommandKind::from_label(&failure.step_label).expect("kind"),
            project_kind: ProjectKind::Cmake,
            combined_log: &failure.combined_log,
            platform: Platform::current(),
            bootstrap_used: failure.bootstrap_used,
        });

        assert!(findings
            .iter()
            .any(|finding| finding.code == "cmake.build_dir_missing"));
    }

    #[cfg(windows)]
    #[test]
    fn execute_plan_calls_bootstrap_batch_with_spaces_before_command() {
        use crate::core::plan::EnvironmentBootstrap;

        let temp = tempfile::tempdir().expect("tempdir");
        let bootstrap_dir = temp.path().join("with spaces");
        std::fs::create_dir_all(&bootstrap_dir).expect("bootstrap dir");
        let marker = temp.path().join("bootstrap-marker.txt");
        let vsdevcmd = bootstrap_dir.join("VsDevCmd.bat");
        std::fs::write(
            &vsdevcmd,
            format!(
                "@echo off\r\necho ok>\"{}\"\r\nexit /b 0\r\n",
                marker.display()
            ),
        )
        .expect("write bootstrap batch");

        let plan = CommandPlan {
            project_root: temp.path().to_path_buf(),
            profile: "debug".to_string(),
            notes: Vec::new(),
            steps: vec![CommandStep {
                label: "bootstrap".to_string(),
                cwd: temp.path().to_path_buf(),
                program: "cmd".to_string(),
                args: vec![
                    "/d".to_string(),
                    "/s".to_string(),
                    "/c".to_string(),
                    "type".to_string(),
                    marker.to_string_lossy().to_string(),
                ],
                env: BTreeMap::new(),
                path_prepend: Vec::new(),
                bootstrap: Some(EnvironmentBootstrap::Msvc {
                    vsdevcmd,
                    arch: "x64".to_string(),
                    host_arch: None,
                }),
            }],
        };

        let outcome = execute_plan(&plan, &RunOptions::default()).expect("execute plan");

        assert_eq!(outcome.last_exit_code, 0);
    }

    #[test]
    fn path_composition_prepends_entries_and_preserves_existing_path() {
        assert_eq!(
            prepend_path_entries(
                &["C:/Qt/bin".to_string(), "C:/tools/bin".to_string()],
                Some("C:/Windows/System32"),
                ';'
            ),
            Some("C:/Qt/bin;C:/tools/bin;C:/Windows/System32".to_string())
        );
    }

    #[test]
    fn path_composition_omits_separator_when_existing_path_is_empty() {
        assert_eq!(
            prepend_path_entries(&["/qt/bin".to_string()], Some(""), ':'),
            Some("/qt/bin".to_string())
        );
    }
}
