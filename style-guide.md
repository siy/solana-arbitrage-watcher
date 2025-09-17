# Solana Arbitrage Watcher – Style & Clarity Guide

This guide codifies conventions and practical recommendations to keep the codebase simple, clear, and consistent. It reconciles current code with the intended standards in README/PLAN/CLAUDE documents and provides concrete, actionable guidance for future changes.

## Objectives
- Prioritize clarity over cleverness; reduce incidental complexity.
- Keep behavior explicit and observable; avoid hidden coupling.
- Prefer small, composable modules with minimal public surface area.

Use this guide as a checklist when touching any file. It captures current pitfalls in the repository and the desired target style to keep the codebase simple and predictable.

## Repository Conventions
- Module boundaries:
  - `websocket/*` – external price sources and reconnection logic
  - `price/*` – data types, cache, validation, normalization
  - `arbitrage/*` – calculation and detection
  - `output/*` – presentation and serialization
  - `config.rs` – CLI parsing and validated config
- Keep cross-module dependencies one-way: websocket → price → arbitrage → output. Avoid cycles.
- One feature per PR; keep diffs small and focused. Delete unused code as part of the feature change.

Layer boundaries must be respected:
- Sources/WebSockets do not format output; they only produce `PriceUpdate` and logs.
- Price layer validates and normalizes; it does not decide arbitrage or print.
- Arbitrage layer computes; it does not log chatty messages, only returns values and optional high-level info logs.
- Output layer formats; it does not perform logic beyond presentation.

## Breaking Inconsistencies To Fix (next edits)
- Remove emojis from all runtime output. CLAUDE/PLAN specify “No emojis”.
- Route human-friendly info to `stdout` and diagnostics/logs to `stderr` via logging, not `println!`.
- Add CLI flag for output format instead of hard-coding `OutputFormat::default()` in `main.rs`.
- Replace scattered `#[allow(dead_code)]` with either usage or `#[cfg(test)]` guards, or remove the code.
- Ensure graceful shutdown of WebSocket tasks when Ctrl+C is received (currently only detection loop aborts).
- Fix build-breakers promptly: `mod util;` is referenced but `src/util` does not exist. Add `src/util/mod.rs` with `format_price_source`, `format_trading_pair`, and `round_to_precision`, or inline them where used.
- Do not synthesize provider URLs from tokens that are insufficient (e.g., QuickNode placeholder). Require full endpoint via `--rpc-url` or a provider-specific `--provider-url` when the scheme cannot be derived from a token.

Additional quick wins:
- Remove stray `println!` calls in tests (e.g., formatter tests) and rely on assertions.
- Normalize log macro usage per module (either `use log::info` or `log::info!`, not both).
- Audit public items and reduce visibility to `pub(crate)` where possible.

## Configuration
- Parse with `clap` to a raw struct, then validate into a “safe” `Config`.
- Expose end-user knobs only through CLI/env; avoid hard-coded policy in modules.
  - Add flags for: `--output-format [table|json|compact]`, `--check-interval-ms`, validation bounds (min/max price), and optional fees.
- Validation rules live in `config.rs`. Keep ranges centralized and documented.
- Avoid panics during config build; return `ConfigErrors` aggregating all issues.

## Data & Semantics
- Spread baseline: choose and document one definition. Current code uses `abs(solana - binance) / binance`.
  - Keep consistent across modules or switch to mid-price/buy-price; test against edge cases.
- Profit threshold: based on net profit after fees, divided by buy price (percentage). Keep invariant and test it.
- Time: prefer `Duration` in APIs and internal state; convert to raw millis only at IO boundaries (CLI, JSON).
- JSON keys: use lower_snake_case consistently; avoid mixing kebab/kebab-case.

## Constructors & Builders
- Prefer a single `new(config)` path; add `with_*` modifiers for optional tuning, returning `Self` (builder-like) or accept a dedicated `*Settings` struct.
- Avoid parallel constructors with overlapping responsibilities (`with_default`, `from_providers`) unless clearly layered.
- Keep `*Config` structs as data; perform validation in `new()` of the main type.

## Error Types
- Mark extensible error enums `#[non_exhaustive]` to allow evolution.
- Include actionable context in messages (provider name/host, pair, bound/threshold values).
- Tests may use `unwrap()`; production code should not.

## Logging & Output
- Adopt a single logging facade consistently:
  - Use `log::{error,warn,info,debug,trace}` in libraries.
  - Initialize a logger in `main` (e.g., `env_logger` or `tracing_subscriber`), mapping levels to `stderr`.
- `output::formatter` controls only final user-facing messages (opportunities / periodic summaries).
- Do not mix logs and formatted output in the same function.
- No emojis. Keep messages short, precise, and machine-greppable.
- Secrets & keys: never log API keys or full provider URLs containing credentials. Prefer logging provider count and types only.
- Prefer a consistent log invocation style project-wide: either `use log::{info,warn,error,...};` and call `info!(...)` or always use `log::info!(...)`. Avoid mixing styles within the same module.
- Keep formatted user-facing output strictly in `OutputFormatter`; other layers return values or log diagnostic context only.
- Avoid bare `println!()` for spacing; incorporate spacing/separators within formatted strings.

## Concurrency & Shutdown
- Don’t hold blocking locks across `.await` points.
- Prefer non-blocking state updates around `PriceCache` (current `std::sync::RwLock` is acceptable since it’s not held across await; if that changes, switch to `tokio::sync::RwLock`).
- Provide explicit shutdown handles:
  - `ConnectionManager::start` should return task handles or a `Shutdown` handle to stop clients cleanly on Ctrl+C.
  - Propagate join failures as errors up to `main` or log at error level.
- Consider `tokio::select!` to drive detection and listen for shutdown in a single task rather than spawn + abort.
- If using `JoinHandle::abort`, ensure tasks do not hold critical locks; prefer a cooperative shutdown path if later complexity grows (close sockets, flush channels, then return).

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
- Consider injecting a time source (`Clock` trait) for tests to avoid flakiness with `SystemTime::now()`.

## Arbitrage
- Keep `FeeCalculator` policy-driven by config (trading fees, gas, default amount). Provide Defaults but let `Config` override.
- Avoid duplicating pair-to-symbol logic across `calculator` and `formatter`. Provide a single helper.
- `ArbitrageDetector` should:
  - Have a single loop interval from config.
  - Expose `check_once()` for tests and `run()` for the loop; keep both small.
  - Update and expose a read-only `DetectionStats` snapshot; no internal printing.
  - Use `Instant`-based timing for internal intervals; avoid `SystemTime` for elapsed checks.

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
- Prefer deterministic backoff tests; current jitter is hash-based and deterministic—keep it that way.
- Add integration tests (under `tests/`) that mock both sources and assert end-to-end formatting and threshold behavior.

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

Candidate removals right now: `borsh`, `base64` (not referenced). Remove unless landing related code.

## Module Checklists

Main (`src/main.rs`)
- Initialize logging first; never print configuration errors—log them.
- Do not log secrets or full URLs; log counts and provider types only.
- Select `OutputFormatter` from CLI-configured `OutputFormat`.
- Manage task lifetimes explicitly (detector, websockets); on Ctrl+C, stop all cleanly.

Config (`src/config.rs`)
- Keep `RawConfig` (CLI) → `Config` (validated) separation.
- Centralize all bounds and defaults here (thresholds, max age, price bounds, intervals, fees if added).
- Prefer explicit provider URLs for API-keyed providers (don’t guess endpoints from tokens).
- Keep error aggregation via `ConfigErrors` with actionable messages.

WebSockets (`src/websocket/*`)
- No `println!`; use `log` macros only.
- Reconnection policy isolated in `ReconnectHandler`.
- Clients emit `PriceUpdate` only; no formatting.
- If adding real Solana parsing, hide mock behind a feature/flag.

Price (`src/price/*`)
- `PriceCache` only stores latest values; no policy.
- `PriceProcessor` validates freshness and bounds from `Config`.
- Avoid holding locks across `.await` boundaries.

Arbitrage (`src/arbitrage/*`)
- `FeeCalculator` configurable via `Config` (fees, default trade size).
- `ArbitrageDetector` owns interval, reads from `PriceProcessor`, produces opportunities.
- Stats struct stays internal to detector; exposed via immutable snapshot.

Output (`src/output/*`)
- Only presentation logic; no side effects beyond returning strings.
- Compact JSON for high-frequency output; pretty JSON only for debugging.
- Include optional per-source timestamps in JSON for traceability (e.g., `solana_ts_ms`, `binance_ts_ms`).

## Pitfalls Detected (as of latest scan)
- Missing `src/util` module referenced by imports; add it or inline helpers.
- QuickNode URL synthesized from token (placeholder). Require full endpoint.
- Mixed log macro styles across modules.
- Widespread `#[allow(dead_code)]`; replace with `#[cfg(test)]` or remove.
- Tests contain `println!` (formatter tests). Remove for quiet test runs.


## CLI & UX
- Add flags for:
  - `--output-format [table|json|compact]`
  - `--check-interval-ms <u64>`
  - `--max-price-age-ms <u64>` (already present)
  - optional: `--min-price <f64>` / `--max-price <f64>` for validation bounds
  - optional: `--fees-json <path>` to inject fee profiles
- Respect `stdout` for user-facing summaries/opportunities; `stderr` for logs.

## Docs & Developer Experience
- Keep README usage examples in sync with CLI flags and defaults.
- Document environment variables and secrets handling explicitly.
- Add short module-level docs where public APIs are exposed (what it does, what it returns, failure modes).

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
