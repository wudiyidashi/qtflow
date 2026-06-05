# QtFlow Diagnostics Contract

## Purpose

Diagnostics should turn common raw build failures into actionable next steps. Diagnostics are not a replacement for compiler output; they summarize the likely setup issue and preserve evidence.

## Diagnostic Output Shape

Human output:

```text
error: build failed

diagnostic: MSVC standard headers are unavailable
evidence:
  fatal error C1083: cannot open include file: 'string'
why:
  cmake was probably executed outside a Visual Studio Developer Command Prompt.
fix:
  qtflow build <target>
  or set QTFLOW_VSDEVCMD_BAT / VSDEVCMD_BAT to VsDevCmd.bat
```

JSON output:

```json
{
  "exitCode": 1,
  "diagnostics": [
    {
      "code": "msvc.missing_standard_headers",
      "severity": "error",
      "title": "MSVC standard headers are unavailable",
      "evidence": ["fatal error C1083: cannot open include file: 'string'"],
      "explanation": "The build likely ran without Visual Studio developer environment variables.",
      "suggestedCommands": [
        "qtflow doctor",
        "set QTFLOW_VSDEVCMD_BAT=<path-to-VsDevCmd.bat>",
        "qtflow build <target>"
      ]
    }
  ]
}
```

## Rule Categories

### MSVC Environment

Code: `msvc.missing_standard_headers`

Patterns:

```text
fatal error C1083: cannot open include file: 'string'
fatal error C1083: cannot open include file: 'vector'
fatal error C1083: cannot open include file: 'stddef.h'
```

Likely cause:

- MSVC developer environment not initialized.

Fix:

- Run through `qtflow`.
- Set `QTFLOW_VSDEVCMD_BAT` or `VSDEVCMD_BAT`.
- Run `qtflow doctor`.

### VsDevCmd Not Found

Code: `msvc.vsdevcmd_not_found`

Patterns:

```text
VsDevCmd.bat was not found
```

Fix:

- Install Visual Studio Build Tools with C++ workload.
- Set `QTFLOW_VSDEVCMD_BAT`.
- Run `qtflow doctor --show-known-msvc`.

### Missing Build Directory

Code: `cmake.build_dir_missing`

Patterns:

```text
Error: could not load cache
not a directory
CMakeCache.txt
```

Fix:

```text
qtflow configure --profile <profile>
qtflow build <target>
```

### Missing CMake Preset

Code: `cmake.preset_missing`

Patterns:

```text
No such preset
Could not read presets
```

Fix:

- Check `CMakePresets.json`.
- Override with `--preset`.
- Update `.qtflow.toml`.

### CMake Not Found

Code: `tool.cmake_not_found`

Patterns:

```text
'cmake' is not recognized
No such file or directory
```

Fix:

- Install CMake.
- Add CMake to PATH.
- Set `[tools].cmake` in `.qtflow.toml`.

### CTest Not Found

Code: `tool.ctest_not_found`

Patterns:

```text
'ctest' is not recognized
No such file or directory
```

Fix:

- Install CMake with CTest.
- Set `[tools].ctest`.

### Qt Runtime DLL Missing

Code: `qt.runtime_dll_missing`

Patterns:

```text
Qt6Core.dll was not found
Qt6Gui.dll was not found
The code execution cannot proceed because Qt6
```

Fix:

- Configure `[qt].bin_dir`.
- Add Qt bin directory to PATH.
- Run deployment helper if the project uses one.

### CTest No Tests Matched

Code: `ctest.no_tests_matched`

Patterns:

```text
No tests were found!!!
0 tests failed out of 0
```

Fix:

- Check CTest regex.
- Use `ctest -N` manually or future `qtflow list-tests`.
- Use `qtflow test <regex> --build-target <target>`.

## MVP Rule Set

Required in MVP:

- `msvc.missing_standard_headers`
- `msvc.vsdevcmd_not_found`
- `cmake.build_dir_missing`
- `cmake.preset_missing`
- `tool.cmake_not_found`
- `tool.ctest_not_found`

Optional in MVP:

- `qt.runtime_dll_missing`
- `ctest.no_tests_matched`

