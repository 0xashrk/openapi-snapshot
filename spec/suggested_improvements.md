# Suggested Improvements Spec

## Phase Tracker

| Phase | Scope | Key Tests | Status |
| --- | --- | --- | --- |
| 1 | Harden outline extraction (fail-fast, no panics) | Outline rejects bad shapes; graceful errors surfaced | Done |
| 2 | Fetch UX: richer errors + headers + retry | HTTP errors include status/body; default headers; retry/backoff | Done |
| 3 | Watch resiliency: ctrl-c, backoff, logging | Ctrl+C stops cleanly; backoff reduces spam; URL switch logged | Done |
| 4 | Outline/reduce failure coverage | Unit + integration tests cover malformed specs and conflicts | Done |
| 5 | CI guardrails (fmt/clippy/tests; optional deny) | CI job runs fmt+clippy+tests; cargo-deny optional | Done |

---

## Phase 1: Harden Outline Extraction
- Goal: Replace `unwrap`/`unwrap_or` fallbacks in `src/outline.rs` with explicit `Result` errors so malformed specs fail without panicking.
- Deliverables:
  - Outline extraction returns descriptive `AppError::Outline` messages for invalid shapes (non-object paths/items, missing names, unsupported content types, malformed schemas).
  - No panics from outline code paths.
- Tests:
  - Unit: reject non-object path item; reject non-query parameter; reject missing parameter name; reject unsupported content type; preserve refs and types without panic.
  - Integration: outline profile run against malformed spec returns exit code 3 with outline-specific message.

## Phase 2: Fetch UX Improvements
- Goal: Improve HTTP ergonomics in `src/fetch.rs`.
- Deliverables:
  - Errors include HTTP status and first N bytes of body (trimmed) when non-2xx.
  - Default headers: `Accept: application/json` and a CLI `User-Agent` string.
  - Optional small retry with backoff for transient network errors/timeouts.
- Tests:
  - Unit: header builder sets defaults and preserves custom headers; retry stops after cap.
  - Integration: mock server returns 500 with body snippet; CLI surfaces status + snippet; transient failure then success is retried.

## Phase 3: Watch Mode Resiliency
- Goal: Make watch loop friendlier and less noisy.
- Deliverables:
  - Graceful shutdown on Ctrl+C.
  - Backoff on repeated failures to reduce log spam; resumes on success.
  - Prompt-once behavior retained; log when URL switches from default after prompt.
- Tests:
  - Unit: backoff increments and resets; prompt-once flag prevents repeat prompt; logs emit URL switch marker.
  - Integration: simulate repeated failures then recovery; loop reduces emission rate; Ctrl+C stops loop cleanly.

## Phase 4: Outline/Reduce Failure Coverage
- Goal: Broaden negative tests for outline/reduce conflicts and malformed specs.
- Deliverables:
  - Unit: outline rejects bad shapes (as Phase 1); reduce rejects missing paths/components; outline vs `--reduce` conflict enforced.
  - Integration: CLI returns exit code 3 with clear message for outline misuse and reduce failures.
- Tests:
  - Unit: missing `paths`, missing `components`, empty reduce list, outline conflict with `--reduce` or `--outline-out`.
  - Integration: malformed input fixtures for outline and reduce, asserting exit codes/messages.

## Phase 5: CI Guardrails
- Goal: Add lightweight CI/lint checks.
- Deliverables:
  - CI workflow runs `cargo fmt -- --check`, `cargo clippy --all --benches --tests --examples --all-features -D warnings`, and the integration test suite.
  - Optional `cargo-deny` job for licenses/versions (documented as optional).
- Tests:
  - CI: workflow succeeds on clean tree; clippy fails on intentional lint to prove enforcement (local dry-run).
  - Optional: cargo-deny config passes on current dependencies.
