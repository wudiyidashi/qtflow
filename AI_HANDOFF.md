# AI Handoff Instructions

You are designing a standalone Rust CLI named `qtflow`.

Read documents in this order:

1. `README.md`
2. `prd.md`
3. `architecture.md`
4. `cli-contract.md`
5. `config-contract.md`
6. `diagnostics-contract.md`
7. `python-helper-reference.md`
8. `agent-skill-template.md`

## Design Task

Produce a Rust project design and implementation plan for `qtflow`.

Required outputs:

- crate/module structure;
- command parsing approach;
- config model;
- command plan model;
- Windows MSVC bootstrap strategy;
- diagnostics rule engine;
- test strategy;
- GitHub release strategy;
- npm wrapper strategy;
- MVP milestone plan.

## Constraints

- Do not implement qmake support in MVP.
- Do not replace CMake or CTest.
- Keep command planning testable without requiring Qt or Visual Studio.
- Treat Windows/MSVC reliability as a first-class requirement.
- Keep CLI output useful for both humans and AI agents.

