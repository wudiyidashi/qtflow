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

[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
generator = "Ninja"
configure_args = []
build_args = []
ctest_args = ["--output-on-failure"]

[profiles.debug.env]
QT_LOGGING_RULES = "qt.qml=false"

[profiles.release]
preset = "Qt-Release"
build_dir = "out/build/release"
generator = "Ninja"
configure_args = []
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
```

Fields:

- `default_profile`: profile used when CLI omits `--profile`.

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
- `ninja`: optional executable name or absolute path.

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
- `bin_dir`: optional Qt bin directory to append/prepend to PATH.

MVP should only report these in `doctor` and optionally add `bin_dir` to command env when configured.

### profiles

```toml
[profiles.debug]
preset = "Qt-Debug"
build_dir = "out/build/debug"
generator = "Ninja"
configure_args = []
build_args = []
ctest_args = ["--output-on-failure"]
```

Fields:

- `preset`: CMake preset name.
- `build_dir`: build directory relative to project root unless absolute.
- `generator`: optional generator when not using preset.
- `config_name`: optional build/test configuration for multi-config generators; CLI `--config-name` overrides this.
- `configure_args`: extra args appended to configure command.
- `build_args`: extra args appended to build command.
- `ctest_args`: extra args appended to CTest command.

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
QTFLOW_VSDEVCMD_BAT   VsDevCmd path override.
VSDEVCMD_BAT          Compatibility path override.
```
