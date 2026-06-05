use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::core::config::merge::{
    DEBUG_BUILD_DIR, DEBUG_PRESET_NAME, DEBUG_PROFILE_NAME, DEFAULT_CTEST_ARG,
    DEFAULT_PROFILE_NAME, RELEASE_BUILD_DIR, RELEASE_PRESET_NAME, RELEASE_PROFILE_NAME,
};
use crate::core::detect::builddir::{
    classify, AmbiguityWarning, BuildDirRole, DiscoveredBuildDir, Provenance,
};
use crate::core::detect::cmake::PresetInfo;
use crate::core::path::{path_to_slash, serialize_path};

pub const BEGIN_MARKER: &str = "<!-- BEGIN qtflow-build-test (managed by `qtflow init`) -->";
pub const END_MARKER: &str = "<!-- END qtflow-build-test -->";
const DEBUG_CONFIG_NAME: &str = "Debug";
const RELEASE_CONFIG_NAME: &str = "Release";

const SKILL_FRONTMATTER: &str = r#"---
name: qtflow-build-test
description: "Build and test Qt/CMake projects through qtflow. Use when asked to compile, build, run CTest, run focused Qt tests, diagnose MSVC/Qt/CMake setup issues, or avoid rediscovering Visual Studio Developer Command Prompt setup."
---"#;

const SKILL_BODY: &str = r#"# QtFlow Build Test

Use `qtflow` instead of manually reconstructing CMake, CTest, or Visual Studio Developer Prompt commands.

## Standard Flow

1. Inspect the environment when needed:

   ```powershell
   qtflow doctor
   ```

2. Build and run one focused test target:

   ```powershell
   qtflow check <test-target>
   ```

3. If the CTest regex differs from the CMake target:

   ```powershell
   qtflow test <ctest-regex> --build-target <cmake-target>
   ```

4. If the command is uncertain, inspect first:

   ```powershell
   qtflow plan check <test-target>
   ```

## Rules

- Do not run raw `cmake --build` first from a normal Windows shell.
- Prefer `qtflow check <target>` for focused backend changes.
- If no focused test exists, run `qtflow build <affected-target>`.
- Report the exact `qtflow` command used in the final answer.
- If `qtflow` emits diagnostics, follow those suggestions before inventing new environment setup commands.

## Troubleshooting

- Missing MSVC standard headers: run `qtflow doctor`, then retry `qtflow check`.
- Missing build dir: run `qtflow configure --profile debug`.
- CTest regex mismatch: use `qtflow test <regex> --build-target <target>`.
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Codex,
    Cursor,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitOptions {
    pub agents: Vec<AgentSelection>,
    pub all: bool,
    pub force: bool,
    pub no_config: bool,
    pub config_only: bool,
    pub dry_run: bool,
    pub layout: Option<InitLayout>,
    pub build_dir_debug: Option<PathBuf>,
    pub build_dir_release: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSelection {
    Agent(Agent),
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitLayout {
    Vs,
    QtCreator,
    Cli,
    Presets,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitConfigInputs {
    pub discovered_build_dirs: Vec<DiscoveredBuildDir>,
    pub presets: Vec<PresetInfo>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedAgents {
    pub claude: bool,
    pub codex: bool,
    pub cursor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitPlan {
    pub project_root: PathBuf,
    pub detected_agents: DetectedAgents,
    pub actions: Vec<InitAction>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InitActionKind {
    Skill,
    Config,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InitStatus {
    Create,
    SkipExists,
    Overwrite,
    WouldCreate,
    WouldSkipExists,
    WouldOverwrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitAction {
    pub kind: InitActionKind,
    pub agent: Option<Agent>,
    pub path: PathBuf,
    pub status: InitStatus,
    pub source: Option<String>,
    pub content: Option<String>,
    pub operation: WriteOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteOperation {
    Write,
    AppendManagedSection,
    ReplaceManagedSection,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InitJsonAction {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    pub path: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJsonReport {
    #[serde(serialize_with = "serialize_path")]
    pub project_root: PathBuf,
    pub actions: Vec<InitJsonAction>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

pub trait InitFileSystem {
    fn exists(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
    fn read_to_string(&self, path: &Path) -> io::Result<String>;
    fn write_string(&mut self, path: &Path, content: &str) -> io::Result<()>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RealInitFileSystem;

impl InitFileSystem for RealInitFileSystem {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn write_string(&mut self, path: &Path, content: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryInitFileSystem {
    files: BTreeMap<PathBuf, String>,
    dirs: BTreeSet<PathBuf>,
}

impl MemoryInitFileSystem {
    pub fn with_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.dirs.insert(path.into());
        self
    }

    pub fn with_file(mut self, path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
        let path = path.into();
        if let Some(parent) = path.parent() {
            self.dirs.insert(parent.to_path_buf());
        }
        self.files.insert(path, content.into());
        self
    }

    pub fn file(&self, path: &Path) -> Option<&str> {
        self.files.get(path).map(String::as_str)
    }

    pub fn files(&self) -> &BTreeMap<PathBuf, String> {
        &self.files
    }
}

impl InitFileSystem for MemoryInitFileSystem {
    fn exists(&self, path: &Path) -> bool {
        self.files.contains_key(path) || self.dirs.contains(path)
    }

    fn is_dir(&self, path: &Path) -> bool {
        self.dirs.contains(path)
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, path.display().to_string()))
    }

    fn write_string(&mut self, path: &Path, content: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            self.dirs.insert(parent.to_path_buf());
        }
        self.files.insert(path.to_path_buf(), content.to_string());
        Ok(())
    }
}

pub fn detect_agents(root: &Path, fs: &impl InitFileSystem) -> DetectedAgents {
    DetectedAgents {
        claude: fs.is_dir(&root.join(".claude")),
        codex: fs.exists(&root.join("AGENTS.md")) || fs.is_dir(&root.join(".codex")),
        cursor: fs.is_dir(&root.join(".cursor")),
    }
}

pub fn plan_actions(
    root: &Path,
    opts: &InitOptions,
    detected: &DetectedAgents,
    fs: &impl InitFileSystem,
    inputs: &InitConfigInputs,
) -> Result<InitPlan, InitPlanError> {
    let mut actions = Vec::new();
    let mut warnings = Vec::new();

    if !opts.config_only {
        for agent in selected_agents(opts, detected) {
            actions.push(plan_skill_action(root, opts, agent, fs));
        }
    }

    if !opts.no_config {
        let (action, config_warnings) = plan_config_action(root, opts, fs, inputs)?;
        warnings.extend(config_warnings);
        actions.push(action);
    }

    Ok(InitPlan {
        project_root: root.to_path_buf(),
        detected_agents: detected.clone(),
        actions,
        warnings,
    })
}

pub fn apply_plan(plan: &InitPlan, fs: &mut impl InitFileSystem) -> io::Result<Vec<InitAction>> {
    let mut applied = Vec::new();

    for action in &plan.actions {
        let mut applied_action = action.clone();
        match action.operation {
            WriteOperation::None => {}
            WriteOperation::Write => {
                if let Some(content) = &action.content {
                    fs.write_string(&action.path, content)?;
                }
            }
            WriteOperation::AppendManagedSection => {
                if let Some(content) = &action.content {
                    let existing = fs.read_to_string(&action.path)?;
                    let appended = append_managed_section(&existing, content);
                    fs.write_string(&action.path, &appended)?;
                }
            }
            WriteOperation::ReplaceManagedSection => {
                if let Some(content) = &action.content {
                    let existing = fs.read_to_string(&action.path)?;
                    let replaced = replace_managed_section(&existing, content)
                        .unwrap_or_else(|| normalize_newlines(content));
                    fs.write_string(&action.path, &replaced)?;
                }
            }
        }

        if !action.status.is_dry_run() {
            applied_action.content = None;
        }
        applied.push(applied_action);
    }

    Ok(applied)
}

pub fn skill_template() -> String {
    format!("{SKILL_FRONTMATTER}\n\n{SKILL_BODY}\n")
}

pub fn cursor_skill_template() -> String {
    let description = r#""Build and test Qt/CMake projects through qtflow. Use when asked to compile, build, run CTest, run focused Qt tests, diagnose MSVC/Qt/CMake setup issues, or avoid rediscovering Visual Studio Developer Command Prompt setup.""#;
    format!("---\ndescription: {description}\nalwaysApply: false\n---\n\n{SKILL_BODY}\n")
}

pub fn codex_managed_section() -> String {
    format!("{BEGIN_MARKER}\n## qtflow-build-test\n\n{SKILL_BODY}\n{END_MARKER}\n")
}

pub fn starter_config_toml_from_presets(presets: &[String]) -> String {
    let debug_preset = find_preset(presets, DEBUG_PROFILE_NAME).unwrap_or(DEBUG_PRESET_NAME);
    let release_preset = find_preset(presets, RELEASE_PROFILE_NAME).unwrap_or(RELEASE_PRESET_NAME);
    starter_config_toml(InitConfigSelection {
        source: "defaults".to_string(),
        debug: default_profile_selection(
            DEBUG_BUILD_DIR,
            DEBUG_PRESET_NAME,
            Some(debug_preset.to_string()),
        ),
        release: default_profile_selection(
            RELEASE_BUILD_DIR,
            RELEASE_PRESET_NAME,
            Some(release_preset.to_string()),
        ),
        multi_config: false,
        warnings: Vec::new(),
    })
}

fn starter_config_toml(selection: InitConfigSelection) -> String {
    let mut output = String::new();
    output.push_str("# qtflow starter config\n");
    output.push_str(
        "# Generated by `qtflow init`. Edit presets/build dirs to match your project.\n\n",
    );
    output.push_str(&format!("default_profile = \"{DEFAULT_PROFILE_NAME}\"\n"));
    if selection.multi_config {
        output.push_str("\n# multi-config generator: build with --config-name Debug/Release\n");
    }
    output.push_str(&profile_toml(DEBUG_PROFILE_NAME, &selection.debug));
    output.push_str(&profile_toml(RELEASE_PROFILE_NAME, &selection.release));
    output
}

fn profile_toml(name: &str, profile: &InitProfileSelection) -> String {
    let mut output = String::new();
    output.push_str(&format!("\n[profiles.{name}]\n"));
    if let Some(preset) = &profile.preset {
        output.push_str(&format!(
            "# preset source: {}\npreset = \"{}\"\n",
            profile.comment,
            toml_escape(preset)
        ));
    }
    if let Some(warning) = &profile.build_dir_warning {
        output.push_str(&format!("# {warning}\n"));
    }
    format!(
        "# build_dir source: {}\nbuild_dir = \"{}\"\n",
        profile.comment,
        toml_escape(&path_to_slash(&profile.build_dir))
    )
    .chars()
    .for_each(|ch| output.push(ch));
    if let Some(generator) = &profile.generator {
        output.push_str(&format!(
            "# generator source: {}\ngenerator = \"{}\"\n",
            profile.comment,
            toml_escape(generator)
        ));
    }
    if let Some(config_name) = &profile.config_name {
        output.push_str(&format!(
            "# Multi-config generators choose Debug/Release at build/test time.\nconfig_name = \"{}\"\n",
            toml_escape(config_name)
        ));
    }
    output.push_str(&format!("ctest_args = [\"{DEFAULT_CTEST_ARG}\"]\n"));
    output
}

pub fn init_json_report(plan: &InitPlan) -> InitJsonReport {
    InitJsonReport {
        project_root: plan.project_root.clone(),
        actions: plan
            .actions
            .iter()
            .map(|action| InitJsonAction {
                kind: action.kind.json_name().to_string(),
                agent: action.agent.map(|agent| agent.name().to_string()),
                path: path_to_slash(&action.path),
                status: action.status.json_name().to_string(),
                source: action.source.clone(),
                content: (action.kind == InitActionKind::Config)
                    .then(|| action.content.clone())
                    .flatten(),
            })
            .collect(),
        warnings: plan.warnings.clone(),
    }
}

fn selected_agents(opts: &InitOptions, detected: &DetectedAgents) -> Vec<Agent> {
    if opts.all || opts.agents.contains(&AgentSelection::All) {
        return all_agents();
    }

    let explicit = opts
        .agents
        .iter()
        .filter_map(|selection| match selection {
            AgentSelection::Agent(agent) => Some(*agent),
            AgentSelection::All => None,
        })
        .collect::<BTreeSet<_>>();

    if !explicit.is_empty() {
        return explicit.into_iter().collect();
    }

    let mut agents = Vec::new();
    if detected.claude {
        agents.push(Agent::Claude);
    }
    if detected.codex {
        agents.push(Agent::Codex);
    }
    if detected.cursor {
        agents.push(Agent::Cursor);
    }
    agents
}

fn all_agents() -> Vec<Agent> {
    vec![Agent::Claude, Agent::Codex, Agent::Cursor]
}

fn plan_skill_action(
    root: &Path,
    opts: &InitOptions,
    agent: Agent,
    fs: &impl InitFileSystem,
) -> InitAction {
    let path = skill_path(root, agent);
    let content = match agent {
        Agent::Claude => skill_template(),
        Agent::Codex => codex_managed_section(),
        Agent::Cursor => cursor_skill_template(),
    };
    let exists = fs.exists(&path);
    let codex_marker_exists = agent == Agent::Codex
        && exists
        && fs
            .read_to_string(&path)
            .map(|existing| existing.contains(BEGIN_MARKER))
            .unwrap_or(false);
    let codex_append = agent == Agent::Codex && exists && !codex_marker_exists;
    let existing_blocks_write =
        exists && !codex_append && (agent != Agent::Codex || codex_marker_exists);
    let will_write = codex_append || !existing_blocks_write || opts.force;

    InitAction {
        kind: InitActionKind::Skill,
        agent: Some(agent),
        path,
        status: if codex_append {
            action_status(false, opts.force, opts.dry_run)
        } else {
            action_status(exists, opts.force, opts.dry_run)
        },
        source: None,
        content: will_write.then_some(content),
        operation: if !will_write || opts.dry_run {
            WriteOperation::None
        } else if agent == Agent::Codex && codex_marker_exists {
            WriteOperation::ReplaceManagedSection
        } else if codex_append {
            WriteOperation::AppendManagedSection
        } else {
            WriteOperation::Write
        },
    }
}

fn plan_config_action(
    root: &Path,
    opts: &InitOptions,
    fs: &impl InitFileSystem,
    inputs: &InitConfigInputs,
) -> Result<(InitAction, Vec<String>), InitPlanError> {
    let path = root.join(".qtflow.toml");
    let exists = fs.exists(&path);
    let will_write = !exists || opts.force;
    let selection = select_config(root, opts, inputs)?;
    let warnings = if will_write {
        selection.warnings.clone()
    } else {
        Vec::new()
    };

    Ok((
        InitAction {
            kind: InitActionKind::Config,
            agent: None,
            path,
            status: action_status(exists, opts.force, opts.dry_run),
            source: Some(selection.source.clone()),
            content: will_write.then(|| starter_config_toml(selection)),
            operation: if will_write && !opts.dry_run {
                WriteOperation::Write
            } else {
                WriteOperation::None
            },
        },
        warnings,
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitPlanError {
    message: String,
}

impl InitPlanError {
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for InitPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InitPlanError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InitConfigSelection {
    source: String,
    debug: InitProfileSelection,
    release: InitProfileSelection,
    multi_config: bool,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InitProfileSelection {
    build_dir: PathBuf,
    generator: Option<String>,
    config_name: Option<String>,
    preset: Option<String>,
    comment: String,
    build_dir_warning: Option<String>,
}

fn select_config(
    root: &Path,
    opts: &InitOptions,
    inputs: &InitConfigInputs,
) -> Result<InitConfigSelection, InitPlanError> {
    if opts.build_dir_debug.is_some() || opts.build_dir_release.is_some() {
        return Ok(selection_from_overrides(root, opts, inputs));
    }

    if let Some(layout) = opts.layout {
        return selection_from_layout(root, layout, inputs);
    }

    let discovered = classify(&inputs.discovered_build_dirs);
    if discovered.debug.is_some() || discovered.release.is_some() {
        return Ok(selection_from_discovery(root, discovered, inputs));
    }

    if let Some(selection) = selection_from_presets(root, inputs, "preset") {
        return Ok(selection);
    }

    Ok(selection_from_defaults())
}

fn selection_from_overrides(
    root: &Path,
    opts: &InitOptions,
    inputs: &InitConfigInputs,
) -> InitConfigSelection {
    let fallback =
        select_without_overrides(root, opts, inputs).unwrap_or_else(|_| selection_from_defaults());
    let debug = opts
        .build_dir_debug
        .as_ref()
        .map(|path| override_profile(root, path, fallback.debug.generator.clone(), inputs))
        .unwrap_or(fallback.debug);
    let release = opts
        .build_dir_release
        .as_ref()
        .map(|path| override_profile(root, path, fallback.release.generator.clone(), inputs))
        .unwrap_or(fallback.release);

    InitConfigSelection {
        source: "override".to_string(),
        debug,
        release,
        multi_config: fallback.multi_config,
        warnings: Vec::new(),
    }
}

fn override_profile(
    root: &Path,
    path: &Path,
    generator: Option<String>,
    inputs: &InitConfigInputs,
) -> InitProfileSelection {
    InitProfileSelection {
        build_dir: relative_to_root(root, path),
        generator,
        config_name: None,
        preset: matching_preset_for_dir(root, path, inputs).map(|preset| preset.name.clone()),
        comment: "override".to_string(),
        build_dir_warning: None,
    }
}

fn select_without_overrides(
    root: &Path,
    opts: &InitOptions,
    inputs: &InitConfigInputs,
) -> Result<InitConfigSelection, InitPlanError> {
    let mut next = opts.clone();
    next.build_dir_debug = None;
    next.build_dir_release = None;

    if let Some(layout) = next.layout {
        return selection_from_layout(root, layout, inputs);
    }

    let discovered = classify(&inputs.discovered_build_dirs);
    if discovered.debug.is_some() || discovered.release.is_some() {
        return Ok(selection_from_discovery(root, discovered, inputs));
    }

    if let Some(selection) = selection_from_presets(root, inputs, "preset") {
        return Ok(selection);
    }

    Ok(selection_from_defaults())
}

fn selection_from_layout(
    root: &Path,
    layout: InitLayout,
    inputs: &InitConfigInputs,
) -> Result<InitConfigSelection, InitPlanError> {
    match layout {
        InitLayout::Vs => Ok(selection_from_vs_layout(root, inputs)),
        InitLayout::Cli => Ok(InitConfigSelection {
            source: "layout:cli".to_string(),
            debug: layout_profile("build", Some("Ninja".to_string()), None, "layout:cli"),
            release: layout_profile(
                "build-release",
                Some("Ninja".to_string()),
                None,
                "layout:cli",
            ),
            multi_config: false,
            warnings: Vec::new(),
        }),
        InitLayout::QtCreator => {
            if let Some(mut selection) = selection_from_presets(root, inputs, "layout:qtcreator") {
                selection.source = "layout:qtcreator".to_string();
                Ok(selection)
            } else {
                Ok(InitConfigSelection {
                    source: "layout:qtcreator".to_string(),
                    debug: layout_profile("build/Debug", None, None, "layout:qtcreator"),
                    release: layout_profile("build/Release", None, None, "layout:qtcreator"),
                    multi_config: false,
                    warnings: Vec::new(),
                })
            }
        }
        InitLayout::Presets => {
            selection_from_presets(root, inputs, "layout:presets").ok_or_else(|| InitPlanError {
                message: "--layout presets requires usable non-hidden CMakePresets.json configurePresets with binaryDir".to_string(),
            })
        }
    }
}

fn selection_from_vs_layout(root: &Path, inputs: &InitConfigInputs) -> InitConfigSelection {
    let discovered = classify(&inputs.discovered_build_dirs);
    let debug = vs_dir_for_role(&discovered, BuildDirRole::Debug)
        .map(|dir| {
            profile_from_discovered(root, dir, DEBUG_CONFIG_NAME, inputs, "layout:vs discovered")
        })
        .unwrap_or_else(|| layout_profile("out/build/x64-Debug", None, None, "layout:vs"));
    let release = vs_dir_for_role(&discovered, BuildDirRole::Release)
        .map(|dir| {
            profile_from_discovered(
                root,
                dir,
                RELEASE_CONFIG_NAME,
                inputs,
                "layout:vs discovered",
            )
        })
        .unwrap_or_else(|| layout_profile("out/build/x64-Release", None, None, "layout:vs"));

    InitConfigSelection {
        source: "layout:vs".to_string(),
        multi_config: debug.config_name.is_some() || release.config_name.is_some(),
        debug,
        release,
        warnings: Vec::new(),
    }
}

fn vs_dir_for_role(
    selection: &crate::core::detect::builddir::BuildDirSelection,
    role: BuildDirRole,
) -> Option<&DiscoveredBuildDir> {
    let chosen = match role {
        BuildDirRole::Debug => selection.debug.as_ref(),
        BuildDirRole::Release => selection.release.as_ref(),
    };
    if chosen.is_some_and(|dir| dir.provenance == Provenance::VisualStudio) {
        return chosen;
    }
    selection
        .warnings
        .iter()
        .find(|warning| warning.role == role)
        .and_then(|warning| {
            warning
                .alternates
                .iter()
                .find(|dir| dir.provenance == Provenance::VisualStudio)
        })
}

fn selection_from_discovery(
    root: &Path,
    selection: crate::core::detect::builddir::BuildDirSelection,
    inputs: &InitConfigInputs,
) -> InitConfigSelection {
    let fallback =
        selection_from_presets(root, inputs, "preset").unwrap_or_else(selection_from_defaults);
    let debug = selection
        .debug
        .as_ref()
        .map(|dir| profile_from_discovered(root, dir, DEBUG_CONFIG_NAME, inputs, "discovered"))
        .unwrap_or(fallback.debug);
    let release = selection
        .release
        .as_ref()
        .map(|dir| profile_from_discovered(root, dir, RELEASE_CONFIG_NAME, inputs, "discovered"))
        .unwrap_or(fallback.release);

    let warnings = selection
        .warnings
        .iter()
        .map(init_warning_text)
        .collect::<Vec<_>>();
    let debug = attach_build_dir_warning(debug, BuildDirRole::Debug, &selection.warnings);
    let release = attach_build_dir_warning(release, BuildDirRole::Release, &selection.warnings);

    InitConfigSelection {
        source: "discovered".to_string(),
        debug,
        release,
        multi_config: selection.multi_config,
        warnings,
    }
}

fn profile_from_discovered(
    root: &Path,
    dir: &DiscoveredBuildDir,
    config_name: &str,
    inputs: &InitConfigInputs,
    comment: &str,
) -> InitProfileSelection {
    InitProfileSelection {
        build_dir: dir.path.clone(),
        generator: Some(dir.generator.clone()),
        config_name: dir.multi_config.then(|| config_name.to_string()),
        preset: matching_preset_for_dir(root, &dir.path, inputs).map(|preset| preset.name.clone()),
        comment: comment.to_string(),
        build_dir_warning: None,
    }
}

fn attach_build_dir_warning(
    mut profile: InitProfileSelection,
    role: BuildDirRole,
    warnings: &[AmbiguityWarning],
) -> InitProfileSelection {
    profile.build_dir_warning = warnings
        .iter()
        .find(|warning| warning.role == role)
        .map(build_dir_comment);
    profile
}

fn init_warning_text(warning: &AmbiguityWarning) -> String {
    format!(
        "multiple {} build dirs found: {} (chosen), {}; override with {}",
        warning.role.as_str(),
        path_to_slash(&warning.chosen.path),
        warning
            .alternates
            .iter()
            .map(alternate_display)
            .collect::<Vec<_>>()
            .join(", "),
        warning.hint
    )
}

fn build_dir_comment(warning: &AmbiguityWarning) -> String {
    format!(
        "alternate {} build dirs: {}; override with {}",
        warning.role.as_str(),
        warning
            .alternates
            .iter()
            .map(alternate_display)
            .collect::<Vec<_>>()
            .join(", "),
        warning.hint
    )
}

fn alternate_display(dir: &DiscoveredBuildDir) -> String {
    let mut display = path_to_slash(&dir.path);
    if dir.provenance == Provenance::VisualStudio {
        display.push_str(" [VS]");
    }
    display
}

fn selection_from_presets(
    root: &Path,
    inputs: &InitConfigInputs,
    source: &str,
) -> Option<InitConfigSelection> {
    let usable = inputs
        .presets
        .iter()
        .filter(|preset| preset.binary_dir.is_some())
        .collect::<Vec<_>>();
    if usable.is_empty() {
        return None;
    }

    let debug = pick_debug_preset(&usable).or_else(|| usable.first().copied())?;
    let release = pick_release_preset(&usable)
        .or_else(|| {
            usable
                .iter()
                .find(|preset| preset.name != debug.name)
                .copied()
        })
        .unwrap_or(debug);

    Some(InitConfigSelection {
        source: source.to_string(),
        debug: profile_from_preset(root, debug, source),
        release: profile_from_preset(root, release, source),
        multi_config: false,
        warnings: Vec::new(),
    })
}

fn profile_from_preset(root: &Path, preset: &PresetInfo, comment: &str) -> InitProfileSelection {
    InitProfileSelection {
        build_dir: preset
            .binary_dir
            .as_ref()
            .map(|path| relative_to_root(root, path))
            .unwrap_or_else(|| PathBuf::from(DEBUG_BUILD_DIR)),
        generator: None,
        config_name: None,
        preset: Some(preset.name.clone()),
        comment: comment.to_string(),
        build_dir_warning: None,
    }
}

fn selection_from_defaults() -> InitConfigSelection {
    InitConfigSelection {
        source: "defaults".to_string(),
        debug: default_profile_selection(DEBUG_BUILD_DIR, DEBUG_PRESET_NAME, None),
        release: default_profile_selection(RELEASE_BUILD_DIR, RELEASE_PRESET_NAME, None),
        multi_config: false,
        warnings: Vec::new(),
    }
}

fn default_profile_selection(
    build_dir: &str,
    default_preset: &str,
    preset: Option<String>,
) -> InitProfileSelection {
    InitProfileSelection {
        build_dir: PathBuf::from(build_dir),
        generator: None,
        config_name: None,
        preset: Some(preset.unwrap_or_else(|| default_preset.to_string())),
        comment: "defaults - verify for your project".to_string(),
        build_dir_warning: None,
    }
}

fn layout_profile(
    build_dir: &str,
    generator: Option<String>,
    preset: Option<String>,
    comment: &str,
) -> InitProfileSelection {
    InitProfileSelection {
        build_dir: PathBuf::from(build_dir),
        generator,
        config_name: None,
        preset,
        comment: comment.to_string(),
        build_dir_warning: None,
    }
}

fn pick_debug_preset<'a>(presets: &[&'a PresetInfo]) -> Option<&'a PresetInfo> {
    presets
        .iter()
        .copied()
        .find(|preset| preset_build_type_matches(preset, "debug"))
        .or_else(|| {
            presets
                .iter()
                .copied()
                .find(|preset| preset.name.to_ascii_lowercase().contains("debug"))
        })
}

fn pick_release_preset<'a>(presets: &[&'a PresetInfo]) -> Option<&'a PresetInfo> {
    presets
        .iter()
        .copied()
        .find(|preset| preset_build_type_matches(preset, "release"))
        .or_else(|| {
            presets
                .iter()
                .copied()
                .find(|preset| preset.name.to_ascii_lowercase().contains("release"))
        })
        .or_else(|| {
            presets.iter().copied().find(|preset| {
                preset
                    .build_type
                    .as_deref()
                    .map(|value| {
                        let lower = value.to_ascii_lowercase();
                        lower == "relwithdebinfo" || lower == "minsizerel"
                    })
                    .unwrap_or(false)
            })
        })
}

fn preset_build_type_matches(preset: &PresetInfo, expected: &str) -> bool {
    preset
        .build_type
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

fn matching_preset_for_dir<'a>(
    root: &Path,
    build_dir: &Path,
    inputs: &'a InitConfigInputs,
) -> Option<&'a PresetInfo> {
    let wanted = comparable_path(build_dir);
    let wanted_absolute = if build_dir.is_absolute() {
        comparable_path(build_dir)
    } else {
        comparable_path(&root.join(build_dir))
    };
    inputs.presets.iter().find(|preset| {
        preset
            .binary_dir
            .as_ref()
            .map(|path| {
                let candidate = comparable_path(path);
                candidate == wanted || candidate == wanted_absolute
            })
            .unwrap_or(false)
    })
}

fn relative_to_root(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root)
        .ok()
        .map(Path::to_path_buf)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| path.to_path_buf())
}

fn comparable_path(path: &Path) -> String {
    let text = path_to_slash(path);
    let text = text
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_string();
    if cfg!(windows) {
        text.to_ascii_lowercase()
    } else {
        text
    }
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn skill_path(root: &Path, agent: Agent) -> PathBuf {
    match agent {
        Agent::Claude => root.join(".claude/skills/qtflow-build-test/SKILL.md"),
        Agent::Codex => root.join("AGENTS.md"),
        Agent::Cursor => root.join(".cursor/rules/qtflow-build-test.mdc"),
    }
}

fn action_status(exists: bool, force: bool, dry_run: bool) -> InitStatus {
    match (dry_run, exists, force) {
        (true, false, _) => InitStatus::WouldCreate,
        (true, true, false) => InitStatus::WouldSkipExists,
        (true, true, true) => InitStatus::WouldOverwrite,
        (false, false, _) => InitStatus::Create,
        (false, true, false) => InitStatus::SkipExists,
        (false, true, true) => InitStatus::Overwrite,
    }
}

fn replace_managed_section(existing: &str, replacement: &str) -> Option<String> {
    let existing = normalize_newlines(existing);
    let start = existing.find(BEGIN_MARKER)?;
    let end_marker_start = existing[start..].find(END_MARKER)? + start;
    let end = end_marker_start + END_MARKER.len();

    let mut output = String::new();
    output.push_str(&existing[..start]);
    output.push_str(&normalize_newlines(replacement));
    output.push_str(&existing[end..]);

    Some(ensure_trailing_newline(&output))
}

fn append_managed_section(existing: &str, section: &str) -> String {
    let existing = normalize_newlines(existing);
    let mut output = ensure_trailing_newline(&existing);
    if !output.ends_with("\n\n") {
        output.push('\n');
    }
    output.push_str(&normalize_newlines(section));
    ensure_trailing_newline(&output)
}

fn find_preset<'a>(presets: &'a [String], needle: &str) -> Option<&'a str> {
    presets
        .iter()
        .find(|preset| preset.to_ascii_lowercase().contains(needle))
        .map(String::as_str)
}

fn normalize_newlines(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}

fn ensure_trailing_newline(content: &str) -> String {
    if content.ends_with('\n') {
        content.to_string()
    } else {
        format!("{content}\n")
    }
}

impl Agent {
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
        }
    }
}

impl InitActionKind {
    fn json_name(&self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Config => "config",
        }
    }
}

impl InitStatus {
    pub fn json_name(&self) -> &'static str {
        match self {
            Self::Create => "created",
            Self::SkipExists => "skipped",
            Self::Overwrite => "overwritten",
            Self::WouldCreate => "would_create",
            Self::WouldSkipExists => "skipped",
            Self::WouldOverwrite => "overwritten",
        }
    }

    pub fn human_name(&self) -> &'static str {
        match self {
            Self::Create => "created",
            Self::SkipExists => "skipped (exists)",
            Self::Overwrite => "overwritten",
            Self::WouldCreate => "would create",
            Self::WouldSkipExists => "would skip (exists)",
            Self::WouldOverwrite => "would overwrite",
        }
    }

    fn is_dry_run(&self) -> bool {
        matches!(
            self,
            Self::WouldCreate | Self::WouldSkipExists | Self::WouldOverwrite
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/repo")
    }

    fn opts() -> InitOptions {
        InitOptions::default()
    }

    fn action_agents(plan: &InitPlan) -> Vec<Agent> {
        plan.actions
            .iter()
            .filter_map(|action| action.agent)
            .collect()
    }

    fn config_content(plan: &InitPlan) -> &str {
        plan.actions
            .iter()
            .find(|action| action.kind == InitActionKind::Config)
            .and_then(|action| action.content.as_deref())
            .expect("config content")
    }

    fn discovered(path: &str, build_type: &str) -> DiscoveredBuildDir {
        discovered_with_provenance(path, build_type, Provenance::Other)
    }

    fn discovered_with_provenance(
        path: &str,
        build_type: &str,
        provenance: Provenance,
    ) -> DiscoveredBuildDir {
        DiscoveredBuildDir {
            path: PathBuf::from(path),
            build_type: Some(build_type.to_string()),
            generator: "Ninja".to_string(),
            multi_config: false,
            provenance,
        }
    }

    fn multi_config_discovered(path: &str, generator: &str) -> DiscoveredBuildDir {
        DiscoveredBuildDir {
            path: PathBuf::from(path),
            build_type: None,
            generator: generator.to_string(),
            multi_config: true,
            provenance: Provenance::Other,
        }
    }

    #[test]
    fn plan_actions_auto_detects_present_agents_only() {
        let fs = MemoryInitFileSystem::default().with_dir("/repo/.claude");
        let detected = detect_agents(&root(), &fs);

        let plan = plan_actions(
            &root(),
            &opts(),
            &detected,
            &fs,
            &InitConfigInputs::default(),
        )
        .expect("plan");

        assert_eq!(action_agents(&plan), vec![Agent::Claude]);
        assert!(plan
            .actions
            .iter()
            .any(|action| action.kind == InitActionKind::Config));
    }

    #[test]
    fn plan_actions_all_selects_every_supported_agent() {
        let fs = MemoryInitFileSystem::default();
        let detected = DetectedAgents::default();
        let options = InitOptions {
            all: true,
            ..opts()
        };

        let plan = plan_actions(
            &root(),
            &options,
            &detected,
            &fs,
            &InitConfigInputs::default(),
        )
        .expect("plan");

        assert_eq!(
            action_agents(&plan),
            vec![Agent::Claude, Agent::Codex, Agent::Cursor]
        );
    }

    #[test]
    fn plan_actions_agent_cursor_selects_only_cursor() {
        let fs = MemoryInitFileSystem::default();
        let detected = DetectedAgents::default();
        let options = InitOptions {
            agents: vec![AgentSelection::Agent(Agent::Cursor)],
            ..opts()
        };

        let plan = plan_actions(
            &root(),
            &options,
            &detected,
            &fs,
            &InitConfigInputs::default(),
        )
        .expect("plan");

        assert_eq!(action_agents(&plan), vec![Agent::Cursor]);
    }

    #[test]
    fn idempotency_skips_existing_without_force_and_overwrites_with_force() {
        let path = root().join(".claude/skills/qtflow-build-test/SKILL.md");
        let fs = MemoryInitFileSystem::default().with_file(&path, "old");
        let detected = DetectedAgents {
            claude: true,
            ..DetectedAgents::default()
        };

        let plan = plan_actions(
            &root(),
            &opts(),
            &detected,
            &fs,
            &InitConfigInputs::default(),
        )
        .expect("plan");
        assert_eq!(plan.actions[0].status, InitStatus::SkipExists);
        assert_eq!(plan.actions[0].operation, WriteOperation::None);

        let force = InitOptions {
            force: true,
            ..opts()
        };
        let plan = plan_actions(
            &root(),
            &force,
            &detected,
            &fs,
            &InitConfigInputs::default(),
        )
        .expect("plan");
        assert_eq!(plan.actions[0].status, InitStatus::Overwrite);
        assert_eq!(plan.actions[0].operation, WriteOperation::Write);
    }

    #[test]
    fn agents_md_created_when_absent() {
        let mut fs = MemoryInitFileSystem::default();
        let detected = DetectedAgents {
            codex: true,
            ..DetectedAgents::default()
        };
        let plan = plan_actions(
            &root(),
            &opts(),
            &detected,
            &fs,
            &InitConfigInputs::default(),
        )
        .expect("plan");

        apply_plan(&plan, &mut fs).expect("apply");

        let agents = fs.file(&root().join("AGENTS.md")).expect("AGENTS.md");
        assert!(agents.contains(BEGIN_MARKER));
        assert!(agents.contains("## qtflow-build-test"));
    }

    #[test]
    fn agents_md_marker_skips_without_force() {
        let content = format!("intro\n{BEGIN_MARKER}\nold\n{END_MARKER}\noutro\n");
        let fs = MemoryInitFileSystem::default().with_file("/repo/AGENTS.md", content);
        let detected = DetectedAgents {
            codex: true,
            ..DetectedAgents::default()
        };

        let plan = plan_actions(
            &root(),
            &opts(),
            &detected,
            &fs,
            &InitConfigInputs::default(),
        )
        .expect("plan");

        assert_eq!(plan.actions[0].status, InitStatus::SkipExists);
        assert_eq!(plan.actions[0].operation, WriteOperation::None);
    }

    #[test]
    fn agents_md_marker_replaces_only_managed_section_with_force() {
        let content = format!("intro\n{BEGIN_MARKER}\nold\n{END_MARKER}\noutro\n");
        let mut fs = MemoryInitFileSystem::default().with_file("/repo/AGENTS.md", content);
        let detected = DetectedAgents {
            codex: true,
            ..DetectedAgents::default()
        };
        let options = InitOptions {
            force: true,
            ..opts()
        };
        let plan = plan_actions(
            &root(),
            &options,
            &detected,
            &fs,
            &InitConfigInputs::default(),
        )
        .expect("plan");

        apply_plan(&plan, &mut fs).expect("apply");

        let agents = fs.file(&root().join("AGENTS.md")).expect("AGENTS.md");
        assert!(agents.starts_with("intro\n"));
        assert!(agents.ends_with("outro\n"));
        assert!(!agents.contains("\nold\n"));
        assert!(agents.contains("qtflow check <test-target>"));
    }

    #[test]
    fn config_inference_maps_debug_and_release_presets() {
        let toml =
            starter_config_toml_from_presets(&["MyDebug".to_string(), "MyRelease".to_string()]);

        assert!(toml.contains(r#"preset = "MyDebug""#));
        assert!(toml.contains(r#"preset = "MyRelease""#));
    }

    #[test]
    fn config_inference_falls_back_to_documented_defaults() {
        let toml = starter_config_toml_from_presets(&[]);

        assert!(toml.contains(r#"default_profile = "debug""#));
        assert!(toml.contains(r#"preset = "Qt-Debug""#));
        assert!(toml.contains(r#"build_dir = "out/build/debug""#));
        assert!(toml.contains(r#"preset = "Qt-Release""#));
        assert!(toml.contains(r#"build_dir = "out/build/release""#));
        assert!(toml.contains("verify for your project"));
    }

    #[test]
    fn config_uses_discovered_build_dirs_by_default() {
        let fs = MemoryInitFileSystem::default();
        let inputs = InitConfigInputs {
            discovered_build_dirs: vec![
                discovered("build", "Debug"),
                discovered("build-release", "Release"),
            ],
            presets: Vec::new(),
        };

        let plan =
            plan_actions(&root(), &opts(), &DetectedAgents::default(), &fs, &inputs).expect("plan");
        let toml = config_content(&plan);

        assert_eq!(plan.actions[0].source.as_deref(), Some("discovered"));
        assert!(toml.contains("# build_dir source: discovered"));
        assert!(toml.contains(r#"build_dir = "build""#));
        assert!(toml.contains(r#"build_dir = "build-release""#));
        assert!(toml.contains(r#"generator = "Ninja""#));
        assert!(!toml.contains(r#"preset = "Qt-Debug""#));
        assert!(!toml.contains("config_name"));
    }

    #[test]
    fn config_sets_config_name_for_discovered_multi_config_build_dir() {
        let fs = MemoryInitFileSystem::default();
        let inputs = InitConfigInputs {
            discovered_build_dirs: vec![multi_config_discovered("build", "Visual Studio 17 2022")],
            presets: Vec::new(),
        };

        let plan =
            plan_actions(&root(), &opts(), &DetectedAgents::default(), &fs, &inputs).expect("plan");
        let toml = config_content(&plan);

        assert_eq!(plan.actions[0].source.as_deref(), Some("discovered"));
        assert!(toml.contains("# Multi-config generators choose Debug/Release at build/test time."));
        assert!(toml.contains(r#"config_name = "Debug""#));
        assert!(toml.contains(r#"config_name = "Release""#));
    }

    #[test]
    fn layout_vs_overrides_discovery() {
        let fs = MemoryInitFileSystem::default();
        let options = InitOptions {
            layout: Some(InitLayout::Vs),
            ..opts()
        };
        let inputs = InitConfigInputs {
            discovered_build_dirs: vec![discovered("build", "Debug")],
            presets: Vec::new(),
        };

        let plan = plan_actions(&root(), &options, &DetectedAgents::default(), &fs, &inputs)
            .expect("plan");
        let toml = config_content(&plan);

        assert_eq!(plan.actions[0].source.as_deref(), Some("layout:vs"));
        assert!(toml.contains(r#"build_dir = "out/build/x64-Debug""#));
        assert!(toml.contains(r#"build_dir = "out/build/x64-Release""#));
        assert!(!toml.contains("generator ="));
    }

    #[test]
    fn layout_vs_uses_discovered_visual_studio_dir() {
        let fs = MemoryInitFileSystem::default();
        let options = InitOptions {
            layout: Some(InitLayout::Vs),
            ..opts()
        };
        let inputs = InitConfigInputs {
            discovered_build_dirs: vec![
                discovered("build", "Debug"),
                discovered_with_provenance("out/build/debug", "Debug", Provenance::VisualStudio),
            ],
            presets: Vec::new(),
        };

        let plan = plan_actions(&root(), &options, &DetectedAgents::default(), &fs, &inputs)
            .expect("plan");
        let toml = config_content(&plan);

        assert_eq!(plan.actions[0].source.as_deref(), Some("layout:vs"));
        assert!(toml.contains(r#"build_dir = "out/build/debug""#));
        assert!(toml.contains(r#"build_dir = "out/build/x64-Release""#));
        assert!(!toml.contains(r#"build_dir = "out/build/x64-Debug""#));
    }

    #[test]
    fn discovered_ambiguity_adds_plan_warning_and_toml_comment() {
        let fs = MemoryInitFileSystem::default();
        let inputs = InitConfigInputs {
            discovered_build_dirs: vec![
                discovered("build", "Debug"),
                discovered_with_provenance("out/build/debug", "Debug", Provenance::VisualStudio),
            ],
            presets: Vec::new(),
        };

        let plan =
            plan_actions(&root(), &opts(), &DetectedAgents::default(), &fs, &inputs).expect("plan");
        let toml = config_content(&plan);

        assert_eq!(
            plan.warnings,
            vec![
                "multiple debug build dirs found: build (chosen), out/build/debug [VS]; override with --build-dir-debug <path>"
                    .to_string()
            ]
        );
        assert!(toml.contains(
            "# alternate debug build dirs: out/build/debug [VS]; override with --build-dir-debug <path>"
        ));
    }

    #[test]
    fn build_dir_debug_override_wins() {
        let fs = MemoryInitFileSystem::default();
        let options = InitOptions {
            layout: Some(InitLayout::Vs),
            build_dir_debug: Some(PathBuf::from("custom-debug")),
            ..opts()
        };
        let inputs = InitConfigInputs {
            discovered_build_dirs: vec![discovered("build", "Debug")],
            presets: Vec::new(),
        };

        let plan = plan_actions(&root(), &options, &DetectedAgents::default(), &fs, &inputs)
            .expect("plan");
        let toml = config_content(&plan);

        assert_eq!(plan.actions[0].source.as_deref(), Some("override"));
        assert!(toml.contains("# build_dir source: override"));
        assert!(toml.contains(r#"build_dir = "custom-debug""#));
        assert!(toml.contains(r#"build_dir = "out/build/x64-Release""#));
    }

    #[test]
    fn layout_presets_errors_without_usable_preset_binary_dirs() {
        let fs = MemoryInitFileSystem::default();
        let options = InitOptions {
            layout: Some(InitLayout::Presets),
            ..opts()
        };

        let err = plan_actions(
            &root(),
            &options,
            &DetectedAgents::default(),
            &fs,
            &InitConfigInputs::default(),
        )
        .expect_err("layout presets should fail");

        assert!(err.message().contains("--layout presets requires"));
    }

    #[test]
    fn existing_qtflow_config_is_skipped() {
        let fs = MemoryInitFileSystem::default().with_file("/repo/.qtflow.toml", "old");
        let detected = DetectedAgents::default();

        let plan = plan_actions(
            &root(),
            &opts(),
            &detected,
            &fs,
            &InitConfigInputs::default(),
        )
        .expect("plan");

        let config = plan
            .actions
            .iter()
            .find(|action| action.kind == InitActionKind::Config)
            .expect("config action");
        assert_eq!(config.status, InitStatus::SkipExists);
        assert_eq!(config.operation, WriteOperation::None);
    }

    #[test]
    fn dry_run_writes_nothing_and_reports_would_create() {
        let mut fs = MemoryInitFileSystem::default().with_dir("/repo/.claude");
        let before = fs.files().clone();
        let detected = detect_agents(&root(), &fs);
        let options = InitOptions {
            dry_run: true,
            ..opts()
        };
        let plan = plan_actions(
            &root(),
            &options,
            &detected,
            &fs,
            &InitConfigInputs::default(),
        )
        .expect("plan");

        apply_plan(&plan, &mut fs).expect("apply");

        assert_eq!(fs.files(), &before);
        assert!(plan
            .actions
            .iter()
            .all(|action| action.status == InitStatus::WouldCreate));
    }
}
