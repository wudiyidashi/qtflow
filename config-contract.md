# QtFlow Config Contract

## File Names

Supported names:

- `.qtflow.toml`
- `qtflow.toml`

Explicit `--config <path>` overrides discovery.

## Merge Precedence

Highest priority first:

1. CLI args
2. Environment variables
3. `.qtflow.toml`
4. inferred defaults

## Minimal Config

```toml
default_profile = "debug"

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"

[profiles.release]
preset = "Qt-Release"
build_dir = "out/build/release"
```

## Full Example

```toml
default_profile = "debug"
build_system = "auto"

[tools]
cmake = "cmake"
ctest = "ctest"
ninja = "ninja"

[msvc]
enabled = true
arch = "x64"
host_arch = "x64"
vsdevcmd = ""

[qt]
root = ""
bin_dir = ""

[qmake]
qmake = ""
spec = ""
make = ""
pro_file = ""
config_args = []

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
generator = "Ninja"
configure_args = []
cache_variables = {}
path_prepend = []
build_args = []
ctest_args = ["--output-on-failure"]

[profiles.debug.env]
QT_LOGGING_RULES = "qt.qml=false"

[profiles.release]
preset = "Qt-Release"
build_dir = "out/build/release"
generator = "Ninja"
configure_args = []
cache_variables = {}
path_prepend = []
build_args = []
ctest_args = ["--output-on-failure"]

[tests.route_dispatcher]
target = "route_dispatcher_request_build_test"
regex = "route_dispatcher_request_build_test"
profile = "debug"

[diagnostics]
enabled = true
max_log_bytes = 200000
```

## Schema

### Root

```toml
default_profile = "debug"
build_system = "auto"
```

Fields:

- `default_profile`: profile used when CLI omits `--profile`.
- `build_system`: `auto`, `cmake`, or `qmake`. `auto` discovers the first ancestor with `CMakeLists.txt` or `*.pro`; CMake wins when both exist in the same directory.

### tools

```toml
[tools]
cmake = "cmake"
ctest = "ctest"
ninja = "ninja"
```

Fields:

- `cmake`: executable name or absolute path.
- `ctest`: executable name or absolute path.
- `ninja`: optional executable name or absolute path. Ninja resolution uses `[tools].ninja`, then `QTFLOW_NINJA`, then `PATH`.

### msvc

```toml
[msvc]
enabled = true
arch = "x64"
host_arch = "x64"
vsdevcmd = ""
```

Fields:

- `enabled`: whether Windows command plans should bootstrap MSVC.
- `arch`: value passed to `VsDevCmd.bat -arch=...`.
- `host_arch`: optional host architecture.
- `vsdevcmd`: explicit path. Empty means auto-detect.

### qt

```toml
[qt]
root = "C:/Qt/6.8.0/msvc2022_64"
bin_dir = "C:/Qt/6.8.0/msvc2022_64/bin"
```

Fields:

- `root`: optional Qt installation root.
- `bin_dir`: optional Qt bin directory prepended to `PATH` for every command step. Relative paths are resolved from the project root.

`doctor` reports these hints. `bin_dir` is also included in the command plan `pathPrepend` field and applied by the runner, which lets CTest subprocesses find Qt runtime DLLs.

### qmake

```toml
[qmake]
qmake = ""
spec = ""
make = ""
pro_file = ""
config_args = []
```

Fields:

- `qmake`: executable name or path. Empty means auto-detect; `QTFLOW_QMAKE` is also honored at command planning/detection time.
- `spec`: qmake mkspec. Empty means `win32-msvc` on Windows/MSVC, otherwise a platform default such as `linux-g++`.
- `make`: make tool. Empty means auto-detect from spec/platform, preferring `nmake`/`jom` for MSVC and `mingw32-make`/`make` otherwise.
- `pro_file`: explicit `.pro` path. Empty means the discovered primary `.pro` file.
- `config_args`: extra arguments appended to the qmake configure command.

### profiles

```toml
[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
generator = "Ninja"
configure_args = []
cache_variables = {}
path_prepend = []
build_args = []
ctest_args = ["--output-on-failure"]
```

Fields:

- `preset`: CMake preset name.
- `build_dir`: build directory relative to project root unless absolute.
- `generator`: optional generator when not using preset.
- `config_name`: optional build/test configuration for multi-config generators; CLI `--config-name` overrides this.
- `cache_variables`: optional CMake cache variables rendered as deterministic sorted `-DKEY=VALUE` arguments.
- `configure_args`: extra args appended to configure command after `cache_variables`; these can override cache variables.
- `path_prepend`: optional list of directories prepended to command `PATH` after `[qt].bin_dir`; relative paths are resolved from the project root.
- `build_args`: extra args appended to build command.
- `ctest_args`: extra args appended to CTest command.

Without a preset, `cache_variables` is the preferred way to inject a vcpkg toolchain or Qt prefix:

```toml
[profiles.debug]
build_dir = "build"
generator = "Ninja"
cache_variables = { CMAKE_TOOLCHAIN_FILE = "C:/vcpkg/scripts/buildsystems/vcpkg.cmake", CMAKE_PREFIX_PATH = "C:/Qt/6.8.0/msvc2022_64" }
```

When the effective generator is exactly `Ninja` and no preset is used, qtflow injects `-DCMAKE_MAKE_PROGRAM=<resolved-ninja>` if Ninja was resolved and `CMAKE_MAKE_PROGRAM` was not already set by `cache_variables` or `configure_args`.

Multi-config generators can set the build/test configuration on the profile:

```toml
[profiles.debug]
build_dir = "out/build/vs"
generator = "Visual Studio 17 2022"
config_name = "Debug"
```

### profile env

```toml
[profiles.debug.env]
KEY = "VALUE"
```

Environment variables merged into command steps.

### tests

```toml
[tests.route_dispatcher]
target = "route_dispatcher_request_build_test"
regex = "route_dispatcher_request_build_test"
profile = "debug"
```

This lets users run:

```text
qtflow check @route_dispatcher
```

MVP optional. If implemented, `@name` resolves to test preset.

## Inferred Defaults

If no config exists:

```toml
default_profile = "debug"

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"

[profiles.release]
preset = "Qt-Release"
build_dir = "out/build/release"
```

If the preset is not found in `CMakePresets.json`, `doctor` should warn. `configure` should fail unless the user provides `--preset` or a config.

## Environment Variables

```text
QTFLOW_CONFIG          Explicit config path.
QTFLOW_PROFILE         Default profile override.
QTFLOW_CMAKE          CMake executable override.
QTFLOW_CTEST          CTest executable override.
QTFLOW_NINJA          Ninja executable override.
QTFLOW_QMAKE          qmake executable override.
QTFLOW_VSDEVCMD_BAT   VsDevCmd path override.
VSDEVCMD_BAT          Compatibility path override.
```

## Unknown Keys

Unknown keys in the root table, `[tools]`, `[msvc]`, `[qt]`, `[qmake]`, `[profiles.*]`, `[diagnostics]`, and `[tests.*]` are ignored but produce warnings such as:

```text
warning: unknown key 'cmake_args' in [profiles.debug] (ignored)
```

`[profiles.*.env]` remains free-form and does not warn on arbitrary environment variable names. `--quiet` suppresses warnings.
