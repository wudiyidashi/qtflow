use super::{CommandKind, Rule, Severity};
use crate::core::project::ProjectKind;

const CONFIGURE_BUILD: &[CommandKind] = &[CommandKind::Configure, CommandKind::Build];
const ALL_COMMANDS: &[CommandKind] = &[
    CommandKind::Configure,
    CommandKind::Build,
    CommandKind::Test,
    CommandKind::Deploy,
];
const BUILD_TEST: &[CommandKind] = &[CommandKind::Build, CommandKind::Test];
const CONFIGURE_ONLY: &[CommandKind] = &[CommandKind::Configure];
const TEST_ONLY: &[CommandKind] = &[CommandKind::Test];
const DEPLOY_ONLY: &[CommandKind] = &[CommandKind::Deploy];
const CMAKE_ONLY: &[ProjectKind] = &[ProjectKind::Cmake];
const QMAKE_ONLY: &[ProjectKind] = &[ProjectKind::Qmake];
const ALL_PROJECTS: &[ProjectKind] = &[ProjectKind::Cmake, ProjectKind::Qmake];

pub static RULES: &[Rule] = &[
    Rule {
        code: "msvc.missing_standard_headers",
        severity: Severity::Error,
        patterns: &[
            "fatal error C1083: cannot open include file: 'string'",
            "fatal error C1083: cannot open include file: 'vector'",
            "fatal error C1083: cannot open include file: 'stddef.h'",
        ],
        applies_to: CONFIGURE_BUILD,
        project_kinds: ALL_PROJECTS,
        title: "MSVC standard headers are unavailable",
        explanation: "The build likely ran without Visual Studio developer environment variables.",
        suggested: &[
            "qtflow doctor",
            "set QTFLOW_VSDEVCMD_BAT=<path-to-VsDevCmd.bat>",
            "qtflow build <target>",
        ],
    },
    Rule {
        code: "msvc.vsdevcmd_not_found",
        severity: Severity::Error,
        patterns: &["VsDevCmd.bat was not found"],
        applies_to: ALL_COMMANDS,
        project_kinds: ALL_PROJECTS,
        title: "VsDevCmd.bat was not found",
        explanation:
            "qtflow could not locate the Visual Studio developer environment bootstrap script.",
        suggested: &[
            "Install Visual Studio Build Tools with C++ workload.",
            "set QTFLOW_VSDEVCMD_BAT=<path-to-VsDevCmd.bat>",
            "qtflow doctor --show-known-msvc",
        ],
    },
    Rule {
        code: "cmake.build_dir_missing",
        severity: Severity::Error,
        patterns: &[
            "Error: could not load cache",
            "not a directory",
            "CMakeCache.txt",
        ],
        applies_to: BUILD_TEST,
        project_kinds: CMAKE_ONLY,
        title: "CMake build directory is missing or not configured",
        explanation: "The selected build directory does not contain a usable CMake cache.",
        suggested: &[
            "qtflow configure --profile <profile>",
            "qtflow build <target>",
        ],
    },
    Rule {
        code: "cmake.preset_missing",
        severity: Severity::Error,
        patterns: &["No such preset", "Could not read presets"],
        applies_to: CONFIGURE_ONLY,
        project_kinds: CMAKE_ONLY,
        title: "CMake preset is missing",
        explanation: "The configured CMake preset was not found in CMakePresets.json.",
        suggested: &[
            "Check CMakePresets.json.",
            "Override with --preset.",
            "Update .qtflow.toml.",
        ],
    },
    Rule {
        code: "tool.cmake_not_found",
        severity: Severity::Error,
        patterns: &["'cmake' is not recognized", "No such file or directory"],
        applies_to: CONFIGURE_BUILD,
        project_kinds: CMAKE_ONLY,
        title: "CMake executable was not found",
        explanation: "qtflow could not start CMake using the configured tool path.",
        suggested: &[
            "Install CMake.",
            "Add CMake to PATH.",
            "Set [tools].cmake in .qtflow.toml.",
        ],
    },
    Rule {
        code: "tool.ctest_not_found",
        severity: Severity::Error,
        patterns: &["'ctest' is not recognized", "No such file or directory"],
        applies_to: TEST_ONLY,
        project_kinds: CMAKE_ONLY,
        title: "CTest executable was not found",
        explanation: "qtflow could not start CTest using the configured tool path.",
        suggested: &["Install CMake with CTest.", "Set [tools].ctest."],
    },
    Rule {
        code: "qt.runtime_dll_missing",
        severity: Severity::Error,
        patterns: &[
            "Qt6Core.dll was not found",
            "Qt6Gui.dll was not found",
            "The code execution cannot proceed because Qt6",
        ],
        applies_to: TEST_ONLY,
        project_kinds: ALL_PROJECTS,
        title: "Qt runtime DLL is missing",
        explanation: "The test executable could not locate the Qt runtime DLLs.",
        suggested: &[
            "Configure [qt].bin_dir.",
            "Add Qt bin directory to PATH.",
            "Run qtflow deploy <target>.",
        ],
    },
    Rule {
        code: "deploy.tool_not_found",
        severity: Severity::Error,
        patterns: &[
            "'windeployqt' is not recognized",
            "windeployqt: command not found",
            "failed to spawn command 'windeployqt'",
            "failed to spawn command 'windeployqt.exe'",
            "'macdeployqt' is not recognized",
            "macdeployqt: command not found",
            "failed to spawn command 'macdeployqt'",
        ],
        applies_to: DEPLOY_ONLY,
        project_kinds: ALL_PROJECTS,
        title: "Qt deployment tool was not found",
        explanation: "qtflow could not start windeployqt or macdeployqt.",
        suggested: &[
            "Set [qt].bin_dir in .qtflow.toml.",
            "Install Qt deployment tools for the selected Qt kit.",
            "qtflow doctor",
        ],
    },
    Rule {
        code: "ctest.no_tests_matched",
        severity: Severity::Warning,
        patterns: &["No tests were found!!!", "0 tests failed out of 0"],
        applies_to: TEST_ONLY,
        project_kinds: CMAKE_ONLY,
        title: "CTest did not find matching tests",
        explanation: "CTest ran, but the selected regex did not match any registered tests.",
        suggested: &[
            "Check CTest regex.",
            "Use ctest -N manually or future qtflow list-tests.",
            "Use qtflow test <regex> --build-target <target>.",
        ],
    },
    Rule {
        code: "qmake.not_found",
        severity: Severity::Error,
        patterns: &[
            "qmake: command not found",
            "'qmake' is not recognized",
            "qmake is not recognized",
            "failed to spawn command",
        ],
        applies_to: CONFIGURE_ONLY,
        project_kinds: QMAKE_ONLY,
        title: "qmake executable was not found",
        explanation: "qtflow could not start qmake using the configured tool path.",
        suggested: &[
            "Set [qmake].qmake in .qtflow.toml.",
            "set QTFLOW_QMAKE=<path-to-Qt-qmake>",
            "qtflow doctor",
        ],
    },
    Rule {
        code: "qmake.conda_qmake",
        severity: Severity::Error,
        patterns: &["Anaconda", "anaconda", "Miniconda", "miniconda", "conda"],
        applies_to: CONFIGURE_ONLY,
        project_kinds: QMAKE_ONLY,
        title: "A conda qmake appears to be selected",
        explanation:
            "The qmake output mentions conda/anaconda and likely comes from the wrong Qt toolchain.",
        suggested: &[
            "qtflow already skips conda qmake during auto-detection.",
            "Set [qmake].qmake to the Qt installation qmake.",
            "set QTFLOW_QMAKE=<path-to-Qt-qmake>",
        ],
    },
    Rule {
        code: "qmake.spec_missing",
        severity: Severity::Error,
        patterns: &[
            "Could not find qmake configuration file",
            "Unknown -spec",
            "QMAKESPEC has not been set",
        ],
        applies_to: CONFIGURE_ONLY,
        project_kinds: QMAKE_ONLY,
        title: "qmake spec is missing or invalid",
        explanation: "qmake could not find the requested mkspec for this Qt installation.",
        suggested: &[
            "Set [qmake].spec, for example win32-msvc.",
            "Install the matching Qt mkspec/toolchain.",
            "qtflow doctor",
        ],
    },
    Rule {
        code: "make.tool_not_found",
        severity: Severity::Error,
        patterns: &[
            "'nmake' is not recognized",
            "'jom' is not recognized",
            "'mingw32-make' is not recognized",
            "nmake: command not found",
            "jom: command not found",
            "mingw32-make: command not found",
            "failed to spawn command 'nmake'",
            "failed to spawn command 'jom'",
            "failed to spawn command 'mingw32-make'",
            "failed to spawn command",
        ],
        applies_to: BUILD_TEST,
        project_kinds: QMAKE_ONLY,
        title: "qmake make tool was not found",
        explanation: "The make tool selected for the qmake spec is not available.",
        suggested: &[
            "Run from a Visual Studio Developer Command Prompt or let qtflow bootstrap MSVC.",
            "Install jom or the matching MinGW make tool.",
            "Set [qmake].make in .qtflow.toml.",
        ],
    },
];
