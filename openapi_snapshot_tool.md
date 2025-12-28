# OpenAPI Snapshot Tool Spec (Crate: openapi-snapshot)

## Phase Index (Status)

| Phase | Subphase | Scope | Key Tests | Status |
| --- | --- | --- | --- | --- |
| 0 | 0.1 | CLI flags + help text | required args parse | Done |
| 0 | 0.2 | Exit codes + errors | missing args -> exit 1 | Done |
| 1 | 1.1 | HTTP fetch | 200 OK + timeout | Done |
| 1 | 1.2 | JSON parse + minify | invalid JSON -> exit 2 | Done |
| 1 | 1.3 | Atomic write | write failure keeps old | Done |
| 2 | 2.1 | Reduction | paths/components only | Done |
| 2 | 2.2 | Reduced validation | missing keys -> exit 3 | Done |
| 3 | 3.1 | Stdout mode | no file created | Done |
| 3 | 3.2 | Docs + examples | help text includes example | Done |
| 3 | 3.3 | Release checklist | cargo publish ready | Done |
| 4 | 4.1 | Watch mode | watch command uses defaults | Done |
| 4 | 4.2 | Default paths + auto mkdir | output dir auto-created | Done |
| 4 | 4.3 | Docs update | README watch section simplified | Done |
| 4 | 4.4 | URL prompt | prompt for port when default fails | Done |
| 5 | 5.1 | Pretty output default | default output is multi-line JSON | Done |
| 5 | 5.2 | Default output path update | default path drops `.min` | Done |
| 5 | 5.3 | Docs + tests | README + tests reflect defaults | Done |
| 6 | 6.1 | Output profiles | add `outline` profile | Done |
| 6 | 6.2 | Outline shape | path + schema refs only | Done |
| 6 | 6.3 | Docs + tests | README + tests reflect outline | Done |
| 7 | 7.1 | Refactor lib | split `src/lib.rs` into modules | Done |
| 7 | 7.2 | Spec + tests | add refactor spec + move unit tests | Done |
| 8 | 8.1 | Dual outputs in watch | watch writes full + outline | Done |
| 8 | 8.2 | CLI flags | `--outline-out`, `--no-outline` | Done |
| 8 | 8.3 | Docs + tests | README + tests reflect dual output | Done |

---

## Summary

Build a small Rust CLI crate named `openapi-snapshot` that fetches an OpenAPI JSON document from a running backend (e.g., utoipa at `/api-docs/openapi.json`) and writes a JSON snapshot suitable for consumption by agents and frontend tooling.

Primary goals:
- Zero-code integration for users who already serve OpenAPI JSON.
- Single command to generate a spec snapshot (pretty by default, minify optional).
- Optional reduction to `paths` and `components` only.
- Optional outline output to keep a smaller agent-friendly file.

Primary use case:
- Keep a local file (e.g., `openapi/backend_openapi.json`) continuously updated during backend development so agents and frontend tooling always have current endpoint inputs/outputs.
- Optionally write an outline file alongside the full snapshot for low-context agents.

---

## Goals

- Provide a dead-simple CLI that works with any OpenAPI JSON URL.
- Produce a JSON snapshot (pretty by default, minify optional).
- Optionally reduce output to `{ paths, components }` only.
- Keep usage obvious to non-Rust users.

## Non-Goals

- Generating OpenAPI from code (that remains app-specific).
- Bundling a backend restarter (users can wire `bacon`, `cargo watch`, or `watchexec`).
- Full OpenAPI validation beyond JSON parse and minimal shape checks.

---

## User Experience

### Install

```
cargo install openapi-snapshot
```

### Basic usage

```
openapi-snapshot --url http://localhost:3000/api-docs/openapi.json --out openapi/backend_openapi.json
```

### Reduced output (paths + components only)

```
openapi-snapshot --url http://localhost:3000/api-docs/openapi.json --out openapi/backend_openapi.json --reduce paths,components
```

### Suggested watcher usage

```
openapi-snapshot watch
```

### Continuous update workflow

One command keeps the files updated while you code:

```
openapi-snapshot watch
```

Leave it running. Every interval:
- the spec is re-fetched,
- `openapi/backend_openapi.json` updates automatically,
- `openapi/backend_openapi.outline.json` stays in sync (unless disabled).

---

## CLI Contract

Commands:
- `openapi-snapshot` (one-shot fetch)
- `openapi-snapshot watch` (poll and refresh on an interval)

Defaults (both commands):
- URL: `http://localhost:3000/api-docs/openapi.json`
- Output: `openapi/backend_openapi.json`
- Minify: `false` (pretty JSON)

Watch defaults:
- Reduce: `paths,components`
- Interval: `2000ms`
- Outline output: `openapi/backend_openapi.outline.json`

Optional flags:
- `--url <string>`: Source OpenAPI JSON URL.
- `--out <path>`: Output path.
- `--outline-out <path>`: Optional outline output path (full profile only).
- `--reduce <list>`: Comma-separated list, supports `paths` and/or `components`.
- `--profile <full|outline>`: Output shape (outline is smaller).
- `--minify` (default false): When set, output is single-line JSON.
- `--timeout-ms <int>`: HTTP timeout.
- `--header <key:value>`: Optional repeated header for auth (e.g., API tokens).
- `--stdout`: Print to stdout instead of file (if set, `--out` is ignored).
- `watch --interval-ms <int>`: Polling interval for refresh.
- `watch --no-outline`: Disable the default outline output file.

Exit codes:
- `0`: success
- `1`: network or HTTP error
- `2`: JSON parse error
- `3`: reduction or schema-shape error
- `4`: filesystem write error

---

## Output Requirements

- Default output: pretty JSON, multi-line.
- Reduced output: JSON containing only:
  - `paths`
  - `components`
- Outline output (when enabled): JSON containing:
  - `paths`
  - `schemas`
- If `--stdout` is set, write to stdout only (no file writes).
- If `--out` is used, write atomically:
  - write to temp file in same directory
  - rename to final path
- Output directories are created automatically if missing.
- Optional output profiles may further reduce payload size (see Phase 6).

---

## Architecture (High Level)

Modules:
- `cli`: argument parsing and help text.
- `fetch`: HTTP GET for the OpenAPI URL.
- `reduce`: optional projection to `{ paths, components }`.
- `minify`: serialize JSON to a single line.
- `write`: atomic file writes.

Dependencies:
- `reqwest` (blocking or async) for HTTP.
- `serde_json` for JSON parsing and serialization.
- `clap` for CLI.

---

## Phased Plan

### Phase 0: CLI Scaffold and Command Shape

Subphases:
- 0.1: Define CLI flags and help text.
- 0.2: Define exit codes and error messages.

Deliverables:
- CLI usage with required `--url` and `--out`.
- Clear help text and examples.

Tests:
Unit:
- CLI parsing accepts required args.
- `--stdout` without `--out` is accepted.
- `--stdout` with `--out` is rejected (or `--out` is ignored) with a clear message.
- `--reduce` rejects unsupported values and mixed-case input.
- `--header` accepts multiple entries and preserves order.

Behavior:
- Missing `--url` or `--out` returns exit code 1 and help text.
- Unknown flags return non-zero and show usage.

### Phase 1: Fetch + Minify + File Write

Subphases:
- 1.1: HTTP fetch with timeout and headers.
- 1.2: JSON parse + minify serialization.
- 1.3: Atomic write to `--out`.

Deliverables:
- Fetch JSON from URL and write minified file.
- Basic error reporting.

Tests:
Integration:
- Fetch from a local test server returns 200 and valid JSON.
- Non-200 response returns exit code 1.
- Network timeout returns exit code 1 with a timeout error.
- DNS failure or connection refused returns exit code 1.

Unit:
- Invalid JSON returns exit code 2.
- Output file contains a single line of JSON.
- Output is valid JSON when parsed again.

Filesystem:
- Unwritable output path returns exit code 4.
- File write is atomic (temp file never left behind on success).
- If write fails mid-way, existing output file remains unchanged.

### Phase 2: Reduction (paths/components only)

Subphases:
- 2.1: Implement `--reduce paths,components`.
- 2.2: Validate reduced structure.

Deliverables:
- Reduced output with only the specified top-level keys.
- No unexpected fields in reduced output.

Tests:
Unit:
- `--reduce paths` outputs only `paths`.
- `--reduce components` outputs only `components`.
- `--reduce paths,components` outputs both.
- Missing `paths` or `components` in input returns exit code 3.
- `--reduce paths,components` preserves nested schemas and refs intact.
- Reduced output is still valid JSON.
- Reduction does not reorder keys in a way that changes semantics.

### Phase 3: UX Polish, Docs, Release

Subphases:
- 3.1: Add `--stdout` mode.
- 3.2: Improve error messages and examples.
- 3.3: Release checklist (versioning, README).

Deliverables:
- `--stdout` prints JSON and skips file write.
- README includes examples for utoipa users.
- Crate published to crates.io.

Tests:
Behavior:
- `--stdout` prints valid JSON and does not create output file.
- CLI returns exit code 0 on success with no stderr output.

Docs:
- Help text includes at least one end-to-end example.
- README includes the watcher workflow and the zero-hook usage.

### Phase 4: Watch Mode + Defaults

Subphases:
- 4.1: Add `watch` subcommand with interval polling.
- 4.2: Provide defaults for URL, output path, and reduction in watch mode.
- 4.3: Simplify README commands to use `openapi-snapshot watch`.
- 4.4: Prompt for port/URL if default endpoint is unreachable in a terminal.

Deliverables:
- `openapi-snapshot watch` runs without flags.
- Output directory is created automatically.
- README shows the short watch command and defaults.
- Default URL failure prompts for a port or full URL (TTY only).

Tests:
- Defaults apply in watch mode (unit test).
- Output directory auto-creation succeeds.
- URL input normalization accepts port, host:port, or full URL.

### Phase 5: Pretty Output Default

Subphases:
- 5.1: Default `--minify` to `false`.
- 5.2: Change default output path to `openapi/backend_openapi.json`.
- 5.3: Update README and tests to match the new defaults.

Deliverables:
- Running `openapi-snapshot` writes a readable JSON file by default.
- Minified output is still available with `--minify true`.
- README and tests describe and validate the new defaults.

Tests:
- Default output contains newlines (pretty JSON).
- `--minify true` produces single-line JSON.

### Phase 6: Outline Output Profile

Goal: produce a compact "endpoint outline" suitable for agents and frontend tooling.

Subphases:
- 6.1: Add `--profile outline` flag (default `full`).
- 6.2: Implement outline shape for paths + schema refs.
- 6.3: Update README + tests to document and validate outline output.

Outline output shape (conceptual):
```
{
  "paths": {
    "/health": {
      "get": {
        "query": [],
        "request": null,
        "responses": {
          "200": "#/components/schemas/HealthResponse"
        }
      }
    }
  },
  "schemas": {
    "HealthResponse": {
      "type": "object",
      "required": ["status"],
      "properties": {
        "status": "string"
      }
    }
  }
}
```

Rules:
- Keep only method + path, query params, request body schema ref, response schema ref.
- For schemas, keep `type`, `required`, and `properties` name + type (or `$ref`).
- Drop descriptions, examples, tags, operationId, format, and extensions.
- `--reduce` is not compatible with `--profile outline`.

Deliverables:
- `openapi-snapshot --profile outline` writes the outline file.
- Output is stable and smaller than full spec.

Tests:
- Outline output omits descriptions and extra metadata.
- Outline output preserves schema refs and required properties.

### Phase 7: lib.rs Refactor

Spec: `spec/refactor_lib_split.md`

Subphases:
- 7.1: Split `src/lib.rs` into focused modules.
- 7.2: Move unit tests into relevant modules.

Deliverables:
- `src/lib.rs` is a thin re-export.
- Tests continue to pass.

### Phase 8: Dual Output Defaults

Subphases:
- 8.1: Watch mode writes both full and outline outputs by default.
- 8.2: Add `--outline-out` and `watch --no-outline`.
- 8.3: Update README and tests for dual output behavior.

Deliverables:
- `openapi-snapshot watch` writes `openapi/backend_openapi.json` and `openapi/backend_openapi.outline.json`.
- Users can override the outline path with `--outline-out` or disable it with `watch --no-outline`.
- Outline output shares the same minify setting as the primary output.

Tests:
- CLI integration test writes both outputs when `--outline-out` is set.
- Config unit test covers default outline output and `--no-outline`.
- README describes dual output defaults and the disable flag.

---

## Compatibility Notes

- Works with any backend that serves OpenAPI JSON over HTTP.
- For utoipa users, default URL is typically `/api-docs/openapi.json`.
- Requires the server to be running if used in fetch mode.

---

## Open Questions

- Should `--minify` default to true (current plan) or require an explicit flag?
- Should `--reduce` default to `paths,components` to keep files minimal?
- Should there be a `--format yaml` option in a later phase?
