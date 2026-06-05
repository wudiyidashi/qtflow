# qtflow

A small Qt/CMake workflow CLI that lets **AI coding agents (and humans) compile and test Qt projects easily** — especially on Windows/MSVC — without rediscovering CMake / CTest / Visual Studio setup every time.

让 AI 方便地编译、测试 Qt（CMake）项目的命令行工具，自动处理 Windows/MSVC 开发者环境。

## Install

```sh
npm i -g qtflow
qtflow --help
```

This installs a prebuilt binary through a platform-specific optional dependency, so **no Rust toolchain is required**. Supported: Windows x64, Linux x64, macOS x64/arm64.

## Quick use

```sh
qtflow doctor          # inspect project, CMake/CTest, build dirs, MSVC
qtflow init            # set up .qtflow.toml + AI agent skills
qtflow check <target>  # build a target and run its matching CTest
```

Full documentation and source: https://github.com/wudiyidashi/qtflow
