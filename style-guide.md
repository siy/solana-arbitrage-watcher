# Solana Arbitrage Watcher – Style & Clarity Guide

This guide codifies conventions and practical recommendations to keep the codebase simple, clear, and consistent. It reconciles current code with the intended standards in README/PLAN/CLAUDE documents and provides concrete, actionable guidance for future changes.

## Objectives
- Prioritize clarity over cleverness; reduce incidental complexity.
- Keep behavior explicit and observable; avoid hidden coupling.
- Prefer small, composable modules with minimal public surface area.

## Repository Conventions
- Module boundaries:
  - `websocket/*` – external price sources and reconnection logic
  - `price/*` – data types, cache, validation, normalization
  - `arbitrage/*` – calculation and detection
  - `output/*` – presentation and serialization
  - `config.rs` – CLI parsing and validated config
- Keep cross-module dependencies one-way: websocket → price → arbitrage → output. Avoid cycles.
- One feature per PR; keep diffs small and focused. Delete unused code as part of the feature change.

## Breaking Inconsistencies To Fix (next edits)
- Remove emojis from all runtime output. CLAUDE/PLAN specify “No emojis”.
- Route human-friendly info to `stdout` and diagnostics/logs to `stderr` via logging, not `println!`.
- Add CLI flag for output format instead of hard-coding `OutputFormat::default()` in `main.rs`.
- Replace scattered `#[allow(dead_code)]` with either usage or `#[cfg(test)]` guards, or remove the code.
- Ensure graceful shutdown of WebSocket tasks when Ctrl+C is received (currently only detection loop aborts).
- Fix build-breakers promptly: `mod util;` is referenced but `src/util` does not exist. Add `src/util/mod.rs` with `format_price_source`, `format_trading_pair`, and `round_to_precision`, or inline them where used.
- Do not synthesize provider URLs from tokens that are insufficient (e.g., QuickNode placeholder). Require full endpoint via `--rpc-url` or a provider-specific `--provider-url` when the scheme cannot be derived from a token.

## Configuration
- Parse with `clap` to a raw struct, then validate into a “safe” `Config`.
- Expose end-user knobs only through CLI/env; avoid hard-coded policy in modules.
  - Add flags for: `--output-format [table|json|compact]`, `--check-interval-ms`, validation bounds (min/max price), and optional fees.
- Validation rules live in `config.rs`. Keep ranges centralized and documented.
- Avoid panics during config build; return `ConfigErrors` aggregating all issues.

## Logging & Output
- Adopt a single logging facade consistently:
  - Use `log::{error,warn,info,debug,trace}` in libraries.
  - Initialize a logger in `main` (e.g., `env_logger` or `tracing_subscriber`), mapping levels to `stderr`.
- `output::formatter` controls only final user-facing messages (opportunities / periodic summaries).
- Do not mix logs and formatted output in the same function.
- No emojis. Keep messages short, precise, and machine-greppable.
- Secrets & keys: never log API keys or full provider URLs containing credentials. Prefer logging provider count and types only.
- Prefer a consistent log invocation style project-wide: either `use log::{info,warn,error,...};` and call `info!(...)` or always use `log::info!(...)`. Avoid mixing styles within the same module.

## Concurrency & Shutdown
- Don’t hold blocking locks across `.await` points.
- Prefer non-blocking state updates around `PriceCache` (current `std::sync::RwLock` is acceptable since it’s not held across await; if that changes, switch to `tokio::sync::RwLock`).
- Provide explicit shutdown handles:
  - `ConnectionManager::start` should return task handles or a `Shutdown` handle to stop clients cleanly on Ctrl+C.
  - Propagate join failures as errors up to `main` or log at error level.
- Consider `tokio::select!` to drive detection and listen for shutdown in a single task rather than spawn + abort.

## WebSocket Clients
- Keep clients pure and side-effect free except for: (1) producing `PriceUpdate`, (2) reconnection decisions.
- All network errors return typed errors; do not `println!` in client code.
- Reconnection policy is encapsulated by `ReconnectHandler`:
  - No sleeps outside of backoff decisions.
  - Reset handler on successful (re)connect.
- Solana client currently mocks price extraction. Gate mock vs real parsing behind a feature flag or config switch (e.g., `--solana-mock`), and isolate mock code paths.

## Price Processing
- `PriceUpdate` is the sole ingress format; keep it stable and well-documented.
- Validate prices once in `PriceProcessor`:
  - Freshness bounds from `Config`.
  - Value bounds configurable (use `Config.price_bounds` not hard-coded numbers).
- `PriceCache` is the minimal thread-safe store of the latest values. Keep it lean; no policy.
- Avoid duplicated helpers across modules (e.g., trading pair display, rounding, time formatting). Consolidate helpers in one place (e.g., `output::util` or `price::util`).

## Arbitrage
- Keep `FeeCalculator` policy-driven by config (trading fees, gas, default amount). Provide Defaults but let `Config` override.
- Avoid duplicating pair-to-symbol logic across `calculator` and `formatter`. Provide a single helper.
- `ArbitrageDetector` should:
  - Have a single loop interval from config.
  - Expose `check_once()` for tests and `run()` for the loop; keep both small.
  - Update and expose a read-only `DetectionStats` snapshot; no internal printing.

## Error Handling
- Each module has a `thiserror` enum for its domain. Use `#[from]` for propagation.
- No `expect`/`unwrap` in runtime paths. Permit only for constant URL parsing or compile-time invariants.
- Error messages include actionable context (which provider, which symbol, which bound failed).

## Testing
- Unit tests close to code; integration tests under `tests/` for end-to-end flows.
- Prefer deterministic tests; avoid time-based flakiness (use injected clocks or small virtualized intervals).
- Remove `#[allow(dead_code)]` patterns by making helpers `#[cfg(test)]` or moving into test modules.
- Test policy boundaries: freshness, validation ranges, threshold checks, reconnection caps.
- Keep tests quiet: avoid `println!` in tests unless debugging; prefer assertions. If logging is useful, initialize with `env_logger::builder().is_test(true).try_init()` and use log macros.

## Naming & API Surface
- Public types and functions require concise doc comments describing contract and failure modes.
- Keep visibility as small as possible. Default to `pub(crate)` rather than `pub` for internal-use APIs.
- Consistent names:
  - `*Config` – inputs and validated settings
  - `*Client` – WebSocket clients
  - `*Processor` – validation/derivation
  - `*Detector` – higher-level orchestration

## Formatting & Lints
- Run `cargo fmt` and `cargo clippy --all-targets -- -D warnings` on every change.
- Do not silence clippy globally. If you need an exception, annotate locally and justify briefly.

## File/Module Hygiene
- No empty modules or placeholder files. Delete unused modules during refactors.
- Shared constants and helpers belong in dedicated `util.rs` where cross-module usage is justified.
- If a module is referenced (e.g., `util::{format_price_source, format_trading_pair, round_to_precision}`), ensure it exists and is tested, or inline helpers locally until the module lands.

## Dependencies Hygiene
- Keep `Cargo.toml` minimal. Remove unused crates (e.g., add `borsh`/`base64` only when code uses them).
- Enable features only as needed (e.g., `tokio-tungstenite` TLS features).
- Run `cargo udeps` periodically to catch unused dependencies (optional, local tooling).

## CLI & UX
- Add flags for:
  - `--output-format [table|json|compact]`
  - `--check-interval-ms <u64>`
  - `--max-price-age-ms <u64>` (already present)
  - optional: `--min-price <f64>` / `--max-price <f64>` for validation bounds
  - optional: `--fees-json <path>` to inject fee profiles
- Respect `stdout` for user-facing summaries/opportunities; `stderr` for logs.

## Migration Checklist (apply as you touch code)
- Replace `println!`/`eprintln!` with `log` macros in libraries; initialize logger in `main`.
- Remove emojis and align messages to short, consistent phrasing.
- Add an `output_format` CLI option and pass it through to `OutputFormatter`.
- Thread shutdown: return or store join handles from `ConnectionManager::start`; cancel them on Ctrl+C.
- Consolidate trading pair symbol mapping in one module.
- Move price validation bounds to config; document defaults.
- Trim `#[allow(dead_code)]` throughout; delete dead code or gate with `#[cfg(test)]`.

## Example Target Shape (high-level)
- `main.rs`:
  - init logger
  - parse raw config → validate `Config`
  - build `ConnectionManager` → start and keep handles
  - build `ArbitrageDetector` with interval from config
  - render via `OutputFormatter` selected by CLI
  - on Ctrl+C: stop detector and shutdown WebSocket tasks

Adhering to these practices will keep the codebase straightforward, maintainable, and predictable as new functionality lands.
