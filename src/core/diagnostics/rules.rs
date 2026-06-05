use super::{CommandKind, Rule, Severity};

const CONFIGURE_BUILD: &[CommandKind] = &[CommandKind::Configure, CommandKind::Build];
const ALL_COMMANDS: &[CommandKind] = &[
    CommandKind::Configure,
    CommandKind::Build,
    CommandKind::Test,
];
const BUILD_TEST: &[CommandKind] = &[CommandKind::Build, CommandKind::Test];
const CONFIGURE_ONLY: &[CommandKind] = &[CommandKind::Configure];
const BUILD_ONLY: &[CommandKind] = &[CommandKind::Build];
const TEST_ONLY: &[CommandKind] = &[CommandKind::Test];

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
        title: "Qt runtime DLL is missing",
        explanation: "The test executable could not locate the Qt runtime DLLs.",
        suggested: &[
            "Configure [qt].bin_dir.",
            "Add Qt bin directory to PATH.",
            "Run deployment helper if the project uses one.",
        ],
    },
    Rule {
        code: "ctest.no_tests_matched",
        severity: Severity::Warning,
        patterns: &["No tests were found!!!", "0 tests failed out of 0"],
        applies_to: TEST_ONLY,
        title: "CTest did not find matching tests",
        explanation: "CTest ran, but the selected regex did not match any registered tests.",
        suggested: &[
            "Check CTest regex.",
            "Use ctest -N manually or future qtflow list-tests.",
            "Use qtflow test <regex> --build-target <target>.",
        ],
    },
];

#[allow(dead_code)]
const _: &[CommandKind] = BUILD_ONLY;
