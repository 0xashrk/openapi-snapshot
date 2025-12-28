# Refactor Spec: Split lib.rs into Modules

## Goal

Reduce `src/lib.rs` size by splitting responsibilities into focused modules while keeping public API stable.

## Scope

- Create new modules: `cli`, `config`, `errors`, `fetch`, `outline`, `output`, `watch`.
- Keep existing CLI behavior and output identical.
- Keep `main.rs` imports working via re-exports in `lib.rs`.
- Move unit tests into the most relevant module.

## Module Plan

```
src/
  cli.rs       # CLI structs, commands, defaults
  config.rs    # Config, ReduceKey, Mode, parsing/validation
  errors.rs    # AppError
  fetch.rs     # HTTP fetch + headers + JSON parsing
  outline.rs   # outline profile logic
  output.rs    # reduce + serialize + atomic write + build_output
  watch.rs     # watch loop + prompt/normalize URL
  lib.rs       # module wiring + re-exports
```

## Public API (Stable)

Re-export from `lib.rs`:
- `Cli`, `Command`, `CommonArgs`, `WatchArgs`, `OutputProfile`
- `Config`, `Mode`, `ReduceKey`, `parse_reduce_list`, `validate_config`
- `AppError`
- `build_output`, `write_output`
- `run_watch`, `maybe_prompt_for_url`

## Tests

- Move unit tests with the functions they cover:
  - `outline.rs`: outline shape tests
  - `output.rs`: reduce + serialization tests
  - `watch.rs`: URL normalization tests
  - `config.rs`: defaults tests
- Keep integration tests in `tests/cli.rs` unchanged.

## Risks

- Module dependency cycles (avoid by keeping CLI types in `cli.rs` and config depending on CLI).
- Missed re-exports causing `main.rs` failures.

## Done When

- `src/lib.rs` is small and only wires modules.
- `cargo test` passes.
- CLI output and flags unchanged.
