use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoverOptions {
    pub max_depth: usize,
}

impl Default for DiscoverOptions {
    fn default() -> Self {
        Self { max_depth: 5 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredBuildDir {
    pub path: PathBuf,
    pub build_type: Option<String>,
    pub generator: String,
    pub multi_config: bool,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    VisualStudio,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildDirRole {
    Debug,
    Release,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbiguityWarning {
    pub role: BuildDirRole,
    pub chosen: DiscoveredBuildDir,
    pub alternates: Vec<DiscoveredBuildDir>,
    pub hint: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildDirSelection {
    pub debug: Option<DiscoveredBuildDir>,
    pub release: Option<DiscoveredBuildDir>,
    pub multi_config: bool,
    pub alternates: Vec<DiscoveredBuildDir>,
    pub warnings: Vec<AmbiguityWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildDirEntry {
    pub path: PathBuf,
    pub file_name: OsString,
    pub is_dir: bool,
    pub is_file: bool,
}

pub trait BuildDirFileSystem {
    fn read_dir(&self, path: &Path) -> io::Result<Vec<BuildDirEntry>>;
    fn read_to_string(&self, path: &Path) -> io::Result<String>;
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RealBuildDirFileSystem;

impl BuildDirFileSystem for RealBuildDirFileSystem {
    fn read_dir(&self, path: &Path) -> io::Result<Vec<BuildDirEntry>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            entries.push(BuildDirEntry {
                path: entry.path(),
                file_name: entry.file_name(),
                is_dir: file_type.is_dir(),
                is_file: file_type.is_file(),
            });
        }
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(entries)
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        path.canonicalize()
    }
}

pub fn discover_build_dirs(root: &Path, opts: &DiscoverOptions) -> Vec<DiscoveredBuildDir> {
    discover_build_dirs_with_fs(root, opts, &RealBuildDirFileSystem)
}

pub fn discover_build_dirs_with_fs(
    root: &Path,
    opts: &DiscoverOptions,
    fs: &impl BuildDirFileSystem,
) -> Vec<DiscoveredBuildDir> {
    let root_canonical = fs.canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut cache_files = Vec::new();
    walk_cache_files(fs, root, 0, opts.max_depth, &mut cache_files);

    let mut candidates = Vec::new();
    for cache_file in cache_files {
        if path_has_noise_segment(&cache_file) {
            continue;
        }

        let Some(cache_dir) = cache_file.parent().map(Path::to_path_buf) else {
            continue;
        };
        if immediate_dir_is_noise(&cache_dir) {
            continue;
        }

        let Ok(content) = fs.read_to_string(&cache_file) else {
            continue;
        };
        let parsed = parse_cache(&content);
        let Some(home_dir) = parsed.home_directory else {
            continue;
        };
        if !same_canonical_path(fs, root, &root_canonical, Path::new(&home_dir)) {
            continue;
        }

        let cache_dir_canonical = fs
            .canonicalize(&cache_dir)
            .unwrap_or_else(|_| cache_dir.clone());
        let display_path = relative_path(&cache_dir, root)
            .or_else(|| relative_path(&cache_dir_canonical, &root_canonical))
            .unwrap_or(cache_dir.clone());
        let generator = parsed.generator.unwrap_or_else(|| "<unknown>".to_string());
        let provenance = if parsed.visual_studio_cache_signal {
            Provenance::VisualStudio
        } else {
            Provenance::Other
        };
        candidates.push(Candidate {
            absolute_path: cache_dir_canonical,
            dir: DiscoveredBuildDir {
                path: display_path,
                build_type: parsed.build_type.filter(|value| !value.is_empty()),
                multi_config: is_multi_config_generator(&generator),
                generator,
                provenance,
            },
        });
    }

    candidates.sort_by(|left, right| {
        path_depth(&left.absolute_path)
            .cmp(&path_depth(&right.absolute_path))
            .then_with(|| path_string(&left.absolute_path).cmp(&path_string(&right.absolute_path)))
    });

    let mut kept: Vec<Candidate> = Vec::new();
    for candidate in candidates {
        if kept
            .iter()
            .any(|existing| is_inside(&candidate.absolute_path, &existing.absolute_path))
        {
            continue;
        }
        kept.push(candidate);
    }

    kept.into_iter().map(|candidate| candidate.dir).collect()
}

pub fn classify(dirs: &[DiscoveredBuildDir]) -> BuildDirSelection {
    let mut multi = Vec::new();
    let mut exact_debug = Vec::new();
    let mut exact_release = Vec::new();
    let mut name_debug = Vec::new();
    let mut name_release = Vec::new();
    let mut releaseish = Vec::new();

    for dir in dirs {
        if dir.multi_config {
            multi.push(dir.clone());
            continue;
        }

        match dir
            .build_type
            .as_deref()
            .map(|value| value.to_ascii_lowercase())
        {
            Some(value) if value == "debug" => exact_debug.push(dir.clone()),
            Some(value) if value == "release" => exact_release.push(dir.clone()),
            Some(value) if value == "relwithdebinfo" || value == "minsizerel" => {
                releaseish.push(dir.clone())
            }
            _ => {
                let name = path_string(&dir.path).to_ascii_lowercase();
                if name.contains("debug") {
                    name_debug.push(dir.clone());
                }
                if name.contains("release") {
                    name_release.push(dir.clone());
                }
            }
        }
    }

    let mut debug_candidates = if exact_debug.is_empty() {
        name_debug
    } else {
        exact_debug
    };
    let mut release_candidates = if !exact_release.is_empty() {
        exact_release
    } else if !name_release.is_empty() {
        name_release
    } else {
        // RelWithDebInfo and MinSizeRel are release-ish fallbacks only when no
        // plain Release candidate was found, so they never displace Release.
        releaseish
    };

    if debug_candidates.is_empty() {
        debug_candidates = multi.clone();
    }
    if release_candidates.is_empty() {
        release_candidates = multi;
    }

    let debug = pick_best(&debug_candidates);
    let release = pick_best(&release_candidates);
    let mut alternates = Vec::new();
    add_alternates(&mut alternates, &debug_candidates, debug.as_ref());
    add_alternates(&mut alternates, &release_candidates, release.as_ref());
    let mut warnings = Vec::new();
    if let Some(warning) = ambiguity_warning(BuildDirRole::Debug, &debug_candidates, debug.as_ref())
    {
        warnings.push(warning);
    }
    if let Some(warning) =
        ambiguity_warning(BuildDirRole::Release, &release_candidates, release.as_ref())
    {
        warnings.push(warning);
    }

    let multi_config = debug.as_ref().is_some_and(|dir| dir.multi_config)
        || release.as_ref().is_some_and(|dir| dir.multi_config);

    BuildDirSelection {
        debug,
        release,
        multi_config,
        alternates,
        warnings,
    }
}

fn walk_cache_files(
    fs: &impl BuildDirFileSystem,
    dir: &Path,
    depth: usize,
    max_depth: usize,
    cache_files: &mut Vec<PathBuf>,
) {
    let Ok(entries) = fs.read_dir(dir) else {
        return;
    };

    for entry in entries {
        if entry.is_file && entry.file_name == "CMakeCache.txt" {
            cache_files.push(entry.path.clone());
        }

        if entry.is_dir && depth < max_depth && !skip_dir_name(&entry.file_name) {
            walk_cache_files(fs, &entry.path, depth + 1, max_depth, cache_files);
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ParsedCache {
    build_type: Option<String>,
    generator: Option<String>,
    home_directory: Option<String>,
    visual_studio_cache_signal: bool,
}

fn parse_cache(content: &str) -> ParsedCache {
    let mut parsed = ParsedCache::default();

    for line in content.lines() {
        if is_visual_studio_generated_by_comment(line) {
            parsed.visual_studio_cache_signal = true;
            continue;
        }

        let Some((key_type, value)) = line.split_once('=') else {
            continue;
        };
        let Some((key, _type_name)) = key_type.split_once(':') else {
            continue;
        };
        match key {
            "CMAKE_BUILD_TYPE" => parsed.build_type = Some(value.trim().to_string()),
            "CMAKE_GENERATOR" => parsed.generator = Some(value.trim().to_string()),
            "CMAKE_GENERATOR_INSTANCE" if is_visual_studio_path(value) => {
                parsed.visual_studio_cache_signal = true;
            }
            "CMAKE_HOME_DIRECTORY" => parsed.home_directory = Some(value.trim().to_string()),
            _ => {}
        }
    }

    parsed
}

fn is_visual_studio_generated_by_comment(line: &str) -> bool {
    const PREFIX: &str = "# It was generated by CMake:";
    let Some(path) = line.trim().strip_prefix(PREFIX) else {
        return false;
    };
    let lower = path.to_ascii_lowercase().replace('\\', "/");
    lower.contains("microsoft visual studio") && lower.contains("commonextensions/microsoft/cmake")
}

fn is_visual_studio_path(value: &str) -> bool {
    value
        .to_ascii_lowercase()
        .contains("microsoft visual studio")
}

fn same_canonical_path(
    fs: &impl BuildDirFileSystem,
    root: &Path,
    root_canonical: &Path,
    other: &Path,
) -> bool {
    let absolute_other = if other.is_absolute() {
        other.to_path_buf()
    } else {
        root.join(other)
    };
    let other_canonical = fs.canonicalize(&absolute_other).unwrap_or(absolute_other);
    path_compare_string(root_canonical) == path_compare_string(&other_canonical)
}

fn relative_path(path: &Path, root: &Path) -> Option<PathBuf> {
    path.strip_prefix(root)
        .ok()
        .map(Path::to_path_buf)
        .filter(|path| !path.as_os_str().is_empty())
}

fn is_multi_config_generator(generator: &str) -> bool {
    generator.starts_with("Visual Studio") || matches!(generator, "Xcode" | "Ninja Multi-Config")
}

fn path_has_noise_segment(path: &Path) -> bool {
    path.components().any(|component| {
        let text = component.as_os_str().to_string_lossy();
        matches!(text.as_ref(), "_deps" | "_build" | "CMakeFiles")
    })
}

fn immediate_dir_is_noise(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("-subbuild") || name.ends_with("-build"))
}

fn skip_dir_name(name: &OsString) -> bool {
    matches!(
        name.to_string_lossy().as_ref(),
        ".git" | "node_modules" | ".trellis" | "target"
    )
}

fn pick_best(candidates: &[DiscoveredBuildDir]) -> Option<DiscoveredBuildDir> {
    let mut candidates = candidates.to_vec();
    candidates.sort_by(|left, right| {
        build_dir_sort_key(left)
            .cmp(&build_dir_sort_key(right))
            .then_with(|| path_string(&left.path).cmp(&path_string(&right.path)))
    });
    candidates.into_iter().next()
}

fn build_dir_sort_key(dir: &DiscoveredBuildDir) -> (bool, usize, String) {
    let path_text = path_string(&dir.path);
    (!is_direct_under_root(&dir.path), path_text.len(), path_text)
}

fn is_direct_under_root(path: &Path) -> bool {
    !path.is_absolute() && path.components().count() == 1
}

fn add_alternates(
    alternates: &mut Vec<DiscoveredBuildDir>,
    candidates: &[DiscoveredBuildDir],
    chosen: Option<&DiscoveredBuildDir>,
) {
    for candidate in candidates {
        if Some(candidate) == chosen {
            continue;
        }
        if !alternates.iter().any(|existing| existing == candidate) {
            alternates.push(candidate.clone());
        }
    }
}

fn ambiguity_warning(
    role: BuildDirRole,
    candidates: &[DiscoveredBuildDir],
    chosen: Option<&DiscoveredBuildDir>,
) -> Option<AmbiguityWarning> {
    if candidates.len() <= 1 {
        return None;
    }
    let chosen = chosen?;
    let mut alternates = Vec::new();
    add_alternates(&mut alternates, candidates, Some(chosen));
    Some(AmbiguityWarning {
        role,
        chosen: chosen.clone(),
        alternates,
        hint: role.override_hint().to_string(),
    })
}

impl BuildDirRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }

    pub fn override_hint(self) -> &'static str {
        match self {
            Self::Debug => "--build-dir-debug <path>",
            Self::Release => "--build-dir-release <path>",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    absolute_path: PathBuf,
    dir: DiscoveredBuildDir,
}

fn is_inside(path: &Path, ancestor: &Path) -> bool {
    path != ancestor && path.starts_with(ancestor)
}

fn path_depth(path: &Path) -> usize {
    path.components()
        .filter(|component| !matches!(component, Component::Prefix(_) | Component::RootDir))
        .count()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn path_compare_string(path: &Path) -> String {
    let text = path_string(path);
    if cfg!(windows) {
        text.to_ascii_lowercase()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_cache(root: &Path, dir: &str, build_type: &str, generator: &str, home: &Path) {
        write_cache_with_prefix(root, dir, build_type, generator, home, "");
    }

    fn write_cache_with_prefix(
        root: &Path,
        dir: &str,
        build_type: &str,
        generator: &str,
        home: &Path,
        prefix: &str,
    ) {
        let build_dir = root.join(dir);
        fs::create_dir_all(&build_dir).expect("build dir");
        fs::write(
            build_dir.join("CMakeCache.txt"),
            format!(
                "{prefix}CMAKE_BUILD_TYPE:STRING={build_type}\nCMAKE_GENERATOR:INTERNAL={generator}\nCMAKE_HOME_DIRECTORY:INTERNAL={}\n",
                home.display()
            ),
        )
        .expect("cache");
    }

    fn canonical(path: &Path) -> PathBuf {
        path.canonicalize().expect("canonical")
    }

    #[test]
    fn discovery_filters_dependency_builds_and_classifies_debug_release() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        fs::write(root.join("CMakeLists.txt"), "").expect("CMakeLists");
        let home = canonical(root);
        let other = tempfile::tempdir().expect("other");

        write_cache(root, "build", "Debug", "Ninja", &home);
        write_cache(root, "build-release", "Release", "Ninja", &home);
        write_cache(
            root,
            "build/_deps/googletest-subbuild",
            "Debug",
            "Ninja",
            other.path(),
        );

        let dirs = discover_build_dirs(root, &DiscoverOptions::default());
        let selection = classify(&dirs);

        assert_eq!(dirs.len(), 2);
        assert!(dirs.iter().any(|dir| dir.path == Path::new("build")));
        assert!(dirs
            .iter()
            .any(|dir| dir.path == Path::new("build-release")));
        assert_eq!(
            selection.debug.as_ref().map(|dir| dir.path.as_path()),
            Some(Path::new("build"))
        );
        assert_eq!(
            selection.release.as_ref().map(|dir| dir.path.as_path()),
            Some(Path::new("build-release"))
        );
    }

    #[test]
    fn multi_config_cache_serves_debug_and_release() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let home = canonical(root);
        write_cache(root, "out/build/vs", "", "Visual Studio 17 2022", &home);

        let dirs = discover_build_dirs(root, &DiscoverOptions::default());
        let selection = classify(&dirs);

        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].multi_config);
        assert_eq!(selection.debug, selection.release);
        assert!(selection.multi_config);
    }

    #[test]
    fn home_directory_mismatch_is_dropped() {
        let temp = tempfile::tempdir().expect("tempdir");
        let other = tempfile::tempdir().expect("other");

        write_cache(temp.path(), "build", "Debug", "Ninja", other.path());

        let dirs = discover_build_dirs(temp.path(), &DiscoverOptions::default());

        assert!(dirs.is_empty());
    }

    #[test]
    fn duplicate_bucket_pick_is_deterministic_and_records_alternates() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let home = canonical(root);
        write_cache(root, "out/build/debug", "Debug", "Ninja", &home);
        write_cache(root, "build-debug", "Debug", "Ninja", &home);

        let dirs = discover_build_dirs(root, &DiscoverOptions::default());
        let selection = classify(&dirs);

        assert_eq!(
            selection.debug.as_ref().map(|dir| dir.path.as_path()),
            Some(Path::new("build-debug"))
        );
        assert_eq!(selection.alternates.len(), 1);
        assert_eq!(
            selection.alternates[0].path,
            PathBuf::from("out/build/debug")
        );
        assert_eq!(selection.warnings.len(), 1);
        assert_eq!(selection.warnings[0].role, BuildDirRole::Debug);
        assert_eq!(
            selection.warnings[0].chosen.path,
            PathBuf::from("build-debug")
        );
        assert_eq!(
            selection.warnings[0].hint,
            "--build-dir-debug <path>".to_string()
        );
    }

    #[test]
    fn detects_visual_studio_provenance_from_generated_by_comment() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let home = canonical(root);
        write_cache_with_prefix(
            root,
            "out/build/debug",
            "Debug",
            "Ninja",
            &home,
            "# It was generated by CMake: C:/Program Files/Microsoft Visual Studio/18/Community/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe\n",
        );
        write_cache_with_prefix(
            root,
            "build",
            "Debug",
            "Ninja",
            &home,
            "# It was generated by CMake: C:/Tools/CMake/bin/cmake.exe\n",
        );

        let dirs = discover_build_dirs(root, &DiscoverOptions::default());

        assert_eq!(
            dirs.iter()
                .find(|dir| dir.path == Path::new("out/build/debug"))
                .map(|dir| dir.provenance),
            Some(Provenance::VisualStudio)
        );
        assert_eq!(
            dirs.iter()
                .find(|dir| dir.path == Path::new("build"))
                .map(|dir| dir.provenance),
            Some(Provenance::Other)
        );
    }

    #[test]
    fn detects_visual_studio_provenance_from_generator_instance() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let home = canonical(root);
        write_cache_with_prefix(
            root,
            "out/build/debug",
            "Debug",
            "Ninja",
            &home,
            "CMAKE_GENERATOR_INSTANCE:INTERNAL=C:/Program Files/Microsoft Visual Studio/18/Community\n",
        );

        let dirs = discover_build_dirs(root, &DiscoverOptions::default());

        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].provenance, Provenance::VisualStudio);
    }

    #[test]
    fn out_build_convention_alone_does_not_set_visual_studio_provenance() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let home = canonical(root);
        write_cache(root, "out/build/debug", "Debug", "Ninja", &home);

        let dirs = discover_build_dirs(root, &DiscoverOptions::default());

        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].provenance, Provenance::Other);
    }

    #[test]
    fn ambiguity_warning_keeps_existing_winner_and_tags_vs_alternate() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let home = canonical(root);
        write_cache(root, "build", "Debug", "Ninja", &home);
        write_cache_with_prefix(
            root,
            "out/build/debug",
            "Debug",
            "Ninja",
            &home,
            "# It was generated by CMake: C:/Program Files/Microsoft Visual Studio/18/Community/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe\n",
        );

        let dirs = discover_build_dirs(root, &DiscoverOptions::default());
        let selection = classify(&dirs);

        assert_eq!(
            selection.debug.as_ref().map(|dir| dir.path.as_path()),
            Some(Path::new("build"))
        );
        assert_eq!(selection.warnings.len(), 1);
        let warning = &selection.warnings[0];
        assert_eq!(warning.role, BuildDirRole::Debug);
        assert_eq!(warning.chosen.path, PathBuf::from("build"));
        assert_eq!(warning.alternates.len(), 1);
        assert_eq!(warning.alternates[0].path, PathBuf::from("out/build/debug"));
        assert_eq!(warning.alternates[0].provenance, Provenance::VisualStudio);
        assert_eq!(warning.hint, "--build-dir-debug <path>");
    }
}
