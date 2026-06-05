# qtflow npm wrapper

This package provides the `qtflow` command by selecting a prebuilt binary from a platform-specific optional dependency.

No Rust toolchain is required when the matching optional package is installed. If the optional package is unavailable, use a GitHub release archive or build from source with `cargo install qtflow`.

The package name is `qtflow`. A scoped package name such as `@qtflow/cli` is also a reasonable future alternative if the project moves to a scoped npm namespace.
