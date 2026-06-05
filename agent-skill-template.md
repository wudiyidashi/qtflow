# Agent Skill Template: QtFlow Build Test

Use this as a starting point for a Codex/Trellis/Claude/Cursor skill in a Qt/CMake project that adopts `qtflow`.

```markdown
---
name: qtflow-build-test
description: "Build and test Qt/CMake projects through qtflow. Use when asked to compile, build, run CTest, run focused Qt tests, diagnose MSVC/Qt/CMake setup issues, or avoid rediscovering Visual Studio Developer Command Prompt setup."
---

# QtFlow Build Test

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
```

