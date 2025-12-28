# openapi-snapshot

Fetch a running backend's OpenAPI JSON and write a minified snapshot for agents and frontend tooling.

## Phases

| Phase | Scope | Status |
| --- | --- | --- |
| 0 | CLI scaffold and command shape | Done |
| 1 | Fetch + minify + file write | Done |
| 2 | Reduction (paths/components only) | Done |
| 3 | UX polish, docs, release | Done |

For subphases and test plans, see `openapi_snapshot_tool.md`.

## What it does

- Fetches OpenAPI JSON from a running server.
- Minifies to a single-line JSON file.
- Optionally reduces to just `paths` and `components`.

## Install

```
cargo install openapi-snapshot
```

## Usage

Requires the OpenAPI URL to be reachable (server running).

Basic:
```
openapi-snapshot --url http://localhost:3000/api-docs/openapi.json --out spec/backend_openapi.min.json
```

Reduce to inputs/outputs only:
```
openapi-snapshot --url http://localhost:3000/api-docs/openapi.json --out spec/backend_openapi.min.json --reduce paths,components
```

Add auth header:
```
openapi-snapshot --url http://localhost:3000/api-docs/openapi.json --out spec/backend_openapi.min.json --header "Authorization: Bearer TOKEN"
```

Print to stdout:
```
openapi-snapshot --url http://localhost:3000/api-docs/openapi.json --stdout
```

## Continuous update

Keep the file updated while you code:

```
cargo watch -x run -x "run --bin openapi-snapshot -- --url http://localhost:3000/api-docs/openapi.json --out spec/backend_openapi.min.json --reduce paths,components"
```

Leave it running. Every change restarts the backend and refreshes the spec file.

## Release checklist

- Update `Cargo.toml` version.
- Run `cargo test`.
- Run `cargo publish`.
- Tag the release in git.

See `openapi_snapshot_tool.md` for the full phased plan, subphases, and test plans.
