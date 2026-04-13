# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

FluxDI is a Rust dependency injection framework supporting transient/singleton/scoped lifecycles, async factories, circular dependency detection, and web framework integration (Axum, Actix). It consists of two crates: `fluxdi` (main library) and `fluxdi-macros` (proc-macro `#[derive(Injectable)]`).

## Build & Test Commands

```bash
cargo build                                    # Build default (fluxdi only, per default-members)
cargo test --all-features                      # Run all tests with all features enabled
cargo test -p fluxdi                           # Test main crate only
cargo test -p fluxdi-macros                    # Test macros crate only
cargo test --test <name>                       # Run a single integration test
cargo test <test_fn_name>                      # Run a single unit test by name
cargo clippy --workspace --all-targets --all-features -- -D warnings  # Lint (CI enforces this)
cargo fmt --check                              # Format check
cargo deny check advisories                    # Security audit
cargo deny check bans licenses sources         # License/dependency policy
cargo doc --no-deps -p fluxdi                  # Build docs
cargo bench                                    # Run Criterion benchmarks
mdbook build docs                              # Build mdBook documentation
RUST_LOG=debug cargo run --example basic       # Run an example with logging
```

## Architecture

### Feature-Gated Thread Safety

The `thread-safe` feature switches the entire runtime type system:
- **Without** `thread-safe`: uses `Rc<RefCell<T>>` (single-threaded)
- **With** `thread-safe`: uses `Arc<RwLock<T>>` (multi-threaded)
- The `lock-free` feature (implies `thread-safe`) swaps `HashMap` for `DashMap`

Type aliases in `runtime.rs` (`Shared<T>`, `Store`) abstract over this. Source files are split by safety mode: `nts_*` (non-thread-safe) and `ts_*` (thread-safe) in the `injector/` module.

### Crate Layout

- **`fluxdi/src/injector/`** — Core DI container (`Injector`). Registration, resolution, graph validation, lifecycle. Files prefixed `nts_` vs `ts_` for thread-safety variants.
- **`fluxdi/src/provider/`** — `Provider<T>` factories with scope management, decorators, resource limiters. Constructor files split by sync/async and thread-safety.
- **`fluxdi/src/application/`** — `Application` bootstrap container with module loading and lifecycle (on_start/on_stop hooks).
- **`fluxdi/src/module/`** — `Module` trait with separate trait definitions for thread-safe vs non-thread-safe.
- **`fluxdi/src/instance/`** — `Instance<T>` wrapper for resolved values.
- **`fluxdi/src/scope.rs`** — `Scope` for per-request service isolation.
- **`fluxdi/src/observability.rs`** — Logging, tracing spans, metrics, Prometheus export, OpenTelemetry bridge.
- **`fluxdi/src/axum.rs`** / **`actix.rs`** — Web framework integration extractors.
- **`fluxdi-macros/src/lib.rs`** — `#[derive(Injectable)]` proc macro.

### Key Feature Flags

`default = ["debug"]`. Notable flags: `thread-safe`, `lock-free`, `async-factory`, `axum` (implies `thread-safe`), `actix` (implies `thread-safe`), `macros`, `tracing`, `metrics`, `prometheus`, `lifecycle`, `resource-limit-async`.

## Code Quality

- Clippy config in `.clippy.toml`: disallows `map_or`, `map_or_else`, `for_each`, `try_for_each`; allows `unwrap`/`expect`/`dbg!`/`print` in tests.
- Dependency policy in `deny.toml`: allowed licenses are MIT, Apache-2.0, BSD-3-Clause, Unicode, Zlib, ISC, CDLA-Permissive-2.0.
- CI runs quality checks on push/PR to main (`.github/workflows/quality.yml`); release via manual dispatch (`.github/workflows/CI.yml`).
- Tests run across Ubuntu/macOS/Windows on stable/beta/nightly.
