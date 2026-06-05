# QtFlow CLI Contract

## Command Overview

```text
qtflow doctor [options]
qtflow plan <command> [command-options]
qtflow configure [options]
qtflow build [target] [options]
qtflow test [regex] [options]
qtflow check <target> [options]
```

## Global Options

```text
--project <path>        Project root or any path inside the project.
--config <path>         Explicit qtflow config file.
--profile <name>        Profile name. Defaults to config default_profile or debug.
--json                  Emit JSON output where supported.
--quiet                 Reduce output.
--verbose               Print detection trace and command details.
--dry-run               Print command plan without executing.
--no-color              Disable ANSI colors.
```

## doctor

Purpose: inspect project and environment.

```text
qtflow doctor
qtflow doctor --json
qtflow doctor --no-probe
```

Options:

```text
--no-probe              Do not execute cmake/ctest/version probes.
--show-known-msvc       Print known VsDevCmd candidate paths.
```

Human output should include:

- project root;
- config file;
- profiles;
- selected build dir;
- CMake path/version;
- CTest path/version;
- CMake presets found;
- MSVC bootstrap status on Windows;
- Qt hints if configured.

## plan

Purpose: render command plan without executing.

```text
qtflow plan configure --profile debug
qtflow plan build app --profile release
qtflow plan check route_dispatcher_request_build_test
```

`plan` should be equivalent to passing `--dry-run`, but it should not require command-specific execution setup.

## configure

Purpose: run CMake configure step.

```text
qtflow configure --profile debug
qtflow configure --preset Qt-Debug
```

Options:

```text
--preset <name>         Override configured CMake preset.
--generator <name>      Optional generator override when not using preset.
--fresh                 Add CMake fresh configure behavior if supported.
--no-msvc-bootstrap     Do not call VsDevCmd on Windows.
--vsdevcmd <path>       Explicit VsDevCmd path.
```

Plan:

```text
cmake --preset <preset>
```

If no preset is configured, later versions may support explicit source/build configure:

```text
cmake -S <root> -B <build_dir> -G <generator>
```

## build

Purpose: build one target or default target.

```text
qtflow build app
qtflow build route_dispatcher_request_build_test --profile debug
qtflow build --all
```

Options:

```text
--build-dir <path>      Override build directory.
--parallel <n>          Pass --parallel N to cmake --build.
--config-name <name>    Build/test configuration for multi-config generators (e.g. Debug, Release).
--all                   Build default/all target.
--no-msvc-bootstrap     Do not call VsDevCmd on Windows.
--vsdevcmd <path>       Explicit VsDevCmd path.
```

Plan:

```text
cmake --build <build_dir> [--config <name>] --target <target> --parallel <n>
```

## test

Purpose: run CTest, optionally building a target first.

```text
qtflow test route_dispatcher_request_build_test
qtflow test route_dispatcher --build-target route_dispatcher_request_build_test
```

Options:

```text
--build-target <target>     Build target before CTest.
--build-dir <path>          Override build directory.
--config-name <name>        Build/test configuration for multi-config generators (e.g. Debug, Release).
--output-on-failure         Default true.
--no-output-on-failure      Disable CTest output-on-failure.
--ctest-arg <arg>           Extra CTest arg. Repeatable.
--parallel <n>              Build parallelism when --build-target is used.
--no-msvc-bootstrap         Do not call VsDevCmd on Windows.
--vsdevcmd <path>           Explicit VsDevCmd path.
```

Plan:

```text
cmake --build <build_dir> [--config <name>] --target <build-target>
ctest --test-dir <build_dir> [-C <name>] -R <regex> --output-on-failure
```

## check

Purpose: build a target and run matching CTest.

```text
qtflow check route_dispatcher_request_build_test
qtflow check app --test-regex smoke
```

Options:

```text
--test-regex <regex>    CTest regex. Defaults to target.
--build-dir <path>      Override build directory.
--config-name <name>    Build/test configuration for multi-config generators (e.g. Debug, Release).
--parallel <n>          Build parallelism.
--ctest-arg <arg>       Extra CTest arg. Repeatable.
--no-msvc-bootstrap     Do not call VsDevCmd on Windows.
--vsdevcmd <path>       Explicit VsDevCmd path.
```

Plan:

```text
cmake --build <build_dir> [--config <name>] --target <target>
ctest --test-dir <build_dir> [-C <name>] -R <target-or-regex> --output-on-failure
```

## Exit Codes

```text
0   Success.
1   Command executed but failed.
2   Configuration or argument error.
3   Required tool not found.
4   Project root/config not found.
5   Environment bootstrap failed.
6   Diagnostic found a known fatal setup issue.
```

## Command Plan JSON

`qtflow plan check foo --json` should emit:

```json
{
  "projectRoot": "D:/repo",
  "profile": "debug",
  "steps": [
    {
      "label": "build",
      "cwd": "D:/repo",
      "program": "cmake",
      "args": ["--build", "D:/repo/out/build/debug", "--target", "foo"],
      "bootstrap": {
        "kind": "msvc",
        "vsdevcmd": "C:/Program Files/Microsoft Visual Studio/18/Community/Common7/Tools/VsDevCmd.bat",
        "arch": "x64"
      }
    },
    {
      "label": "test",
      "cwd": "D:/repo",
      "program": "ctest",
      "args": ["--test-dir", "D:/repo/out/build/debug", "-R", "foo", "--output-on-failure"],
      "bootstrap": {
        "kind": "msvc",
        "vsdevcmd": "C:/Program Files/Microsoft Visual Studio/18/Community/Common7/Tools/VsDevCmd.bat",
        "arch": "x64"
      }
    }
  ]
}
```
