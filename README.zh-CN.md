# qtflow

[English](README.md) | **中文**

qtflow 是一个面向 Qt/CMake 项目的 Rust 命令行工具,用来把日常的 `configure`、`build`、`test`、`check` 工作流标准化。它在底层调用 CMake 和 CTest,自动发现项目 / 配置 / 构建目录上下文,并在 Windows 上按需自动初始化 MSVC 开发者环境。它**不是** CMake、CTest、Qt Creator 或 Visual Studio 的替代品,而是给人和 AI 编码代理使用的一层薄而可预测的工作流封装。

核心理念:**规划是纯函数,执行是薄壳**。每个命令先产出一个可序列化的命令计划(`CommandPlan`),`--dry-run` / `plan` 只渲染不执行;输出对人(文本)和代理(`--json`)都友好,退出码稳定。

## 安装

### npm(推荐,无需 Rust 工具链)

```sh
npm i -g qtflow
qtflow --help
```

npm 包会通过平台专属的可选依赖包自动拉取对应平台的预编译二进制,因此装它不需要 Rust 工具链。

### 从 GitHub Release 下载

到仓库的 Releases 页面下载对应平台的归档并解压,把 `qtflow` / `qtflow.exe` 放到 `PATH`:

```text
qtflow-<version>-x86_64-pc-windows-msvc.zip
qtflow-<version>-x86_64-unknown-linux-gnu.tar.gz
qtflow-<version>-x86_64-apple-darwin.tar.gz
qtflow-<version>-aarch64-apple-darwin.tar.gz
```

### 用 Cargo 从源码安装

```sh
cargo install --git https://github.com/wudiyidashi/qtflow
```

## 快速上手

在 Qt/CMake 项目里运行,或在任意位置用 `--project <路径>` 指定项目:

```powershell
qtflow doctor                       # 体检:项目根、cmake/ctest 版本、发现的构建目录、MSVC 状态
qtflow init                         # 自动注入 agent skill + 生成起步 .qtflow.toml
qtflow configure --profile debug    # 跑 CMake 配置
qtflow build <target>               # 构建某个目标
qtflow check <target>               # ★最常用:先 build 目标,再跑匹配的 CTest
qtflow test <regex> --build-target <target>   # CTest 名与目标名不一致时
```

看不准就先加 `--dry-run` 或用 `qtflow plan <命令>` 只看将要执行的精确命令;加 `--json` 得到机器可读输出。

## 命令一览

| 命令 | 作用 |
|---|---|
| `doctor` | 环境体检(项目、配置、cmake/ctest 版本、构建目录、MSVC) |
| `init` | 注入 agent skill 到 `.claude`/`.codex`/`.cursor` + 生成 `.qtflow.toml` |
| `configure` | 跑 CMake 配置(用 preset,或无 preset 时回退 `-S/-B/-G`) |
| `build` | 构建一个目标或默认目标 |
| `test` | 跑 CTest,可先 `--build-target` 构建 |
| `check` | 构建目标 + 跑匹配的 CTest(代理的默认质量门) |
| `plan` | 只渲染命令计划,不执行 |

## 配置 `.qtflow.toml`

放在项目根。没有它也能用(会推断默认值)。合并优先级:**CLI > 环境变量 > 文件 > 推断默认**。

```toml
default_profile = "debug"

[profiles.debug]
preset = "Qt-Debug"          # 或省略 preset,用 build_dir + generator
build_dir = "out/build/debug"
generator = "Ninja"
ctest_args = ["--output-on-failure"]

[profiles.release]
build_dir = "build-release"
generator = "Ninja"
```

环境变量:`QTFLOW_CONFIG`、`QTFLOW_PROFILE`、`QTFLOW_CMAKE`、`QTFLOW_CTEST`、`QTFLOW_VSDEVCMD_BAT`、`VSDEVCMD_BAT`。

## 构建目录发现与 Visual Studio

`qtflow init` / `doctor` 会扫描 `CMakeCache.txt` 自动发现真实构建目录,读取 `CMAKE_BUILD_TYPE` / `CMAKE_GENERATOR` 分类 debug/release,并过滤掉 `_deps`、子构建等噪声。

由于 VS 的 CMake 默认也用 Ninja,单看生成器分不出谁是 VS,因此 qtflow 会从 cache 识别 **VS 出处**(`out/build/...`)。当同一角色发现多个目录时会给出歧义告警,并可覆盖:

```sh
qtflow init --layout vs            # 优先用发现到的 VS 目录
qtflow init --layout cli           # build / build-release
qtflow init --build-dir-debug out/build/debug      # 显式指定
```

## Windows / MSVC

在 Windows 上,当 `[msvc].enabled = true` 且未传 `--no-msvc-bootstrap` 时,qtflow 会在执行 configure/build/test 前自动初始化 MSVC 环境:解析 `VsDevCmd.bat`,再通过 `cmd.exe` 以 `call "<VsDevCmd.bat>" -arch=<arch>` 包裹命令执行。

`VsDevCmd.bat` 解析优先级:`--vsdevcmd` → `QTFLOW_VSDEVCMD_BAT` → `VSDEVCMD_BAT` → 配置 → `VSINSTALLDIR` → `vswhere` → 已知安装路径。

## AI 代理集成

`qtflow init` 把 repo-scoped 的 `qtflow-build-test` 指引注入项目里已存在的代理:

- Claude → `.claude/skills/qtflow-build-test/SKILL.md`
- Codex → `AGENTS.md`(BEGIN/END 受管段,保留你原有内容)
- Cursor → `.cursor/rules/qtflow-build-test.mdc`

Codex 只会从全局 skills 目录自动加载真正可复用的 skill,不会从项目里的 `.codex/skills/` 自动加载。用 `qtflow init --global` 安装全局 Codex skill 到 `$CODEX_HOME/skills/qtflow-build-test/`,未设置 `CODEX_HOME` 时默认是 `~/.codex/skills/qtflow-build-test/`;它会写入 `SKILL.md` 和 `agents/openai.yaml`,安装后重启 Codex 生效。

默认自动探测;也可 `--agent claude|codex|cursor|all` 指定,`--global` 额外安装全局 Codex skill,`--dry-run` 预览、`--force` 覆盖。这样任意 Qt/CMake 仓库里的 AI 代理都会自动改用 `qtflow` 而不是裸 `cmake`。

## 诊断

命令失败时,qtflow 会输出结构化诊断(文本 + `--json`)并给出修复建议,而不是堆原始日志。覆盖常见问题:MSVC 标准头缺失、`VsDevCmd` 未找到、构建目录缺失、preset 缺失、cmake/ctest 未找到、Qt 运行时 DLL 缺失、CTest 无匹配测试等。

## 退出码

```text
0  成功
1  命令执行了但失败
2  配置 / 参数错误
3  所需工具未找到
4  项目根 / 配置未找到
5  环境引导失败
6  诊断到已知致命的配置问题
```

## JSON 输出

```sh
qtflow plan check <target> --json     # 命令计划(供代理 / CI 消费)
qtflow doctor --json                  # 环境与发现结果
```
