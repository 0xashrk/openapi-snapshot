# openapi-snapshot

> [!NOTE]
>
> "Give agents the backend contract, and they stop guessing. This tool keeps that contract current."

Fetch a running backend's OpenAPI JSON and write a readable snapshot for agents and frontend tooling.

## Overview

`openapi-snapshot` is a small CLI that pulls a live OpenAPI JSON endpoint and writes a readable JSON file. The goal is to keep a lightweight, up-to-date backend contract that agents and frontend tooling can read without guessing.

## What it does

- Fetches OpenAPI JSON from a running server.
- Writes pretty JSON by default.
- Optionally minifies, reduces, or outputs a minimal outline.
- Can keep both a full snapshot and a smaller outline in sync.

## Why this exists

Agents and frontend projects work best when they can see the backend contract. The snapshot file is meant to be referenced from `AGENTS.md`/`CLAUDE.md` or frontend docs so everyone (humans and tools) always has the current endpoints and schemas.

## Recommended usage (agents + frontend)

1) Generate the snapshot file in a predictable location (e.g., `openapi/backend_openapi.json`).
2) Reference it in your `AGENTS.md` or `CLAUDE.md` so agents always load it:

```
Backend contract: openapi/backend_openapi.json
```

3) If you want a much smaller file for agents, also reference the outline file:

```
Backend outline: openapi/backend_openapi.outline.json
```

4) In a separate frontend repo, point to the full snapshot path (relative link or shared mount). This keeps frontend work aligned with the backend contract.

## When to use it

- You want agents or frontend tooling to always have up-to-date endpoint inputs/outputs.
- You already have Swagger/OpenAPI JSON available (e.g., `/api-docs/openapi.json`).

## Install

```
cargo install openapi-snapshot
```

## Usage

Requires the OpenAPI URL to be reachable (server running).

Basic:
```
openapi-snapshot --url http://localhost:3000/api-docs/openapi.json --out openapi/backend_openapi.json
```

Notes:
- This tool does not prompt for a save location. Pass `--out` to choose a path or use the defaults.
- If the output directory does not exist, it will be created automatically.

Quick default (no flags):
```
openapi-snapshot
```
Defaults:
- URL: `http://localhost:3000/api-docs/openapi.json`
- Output: `openapi/backend_openapi.json`
- Minify: `false` (pretty JSON)

If the default URL is unreachable and you're in a terminal, the CLI will prompt you for a port or full URL.

Reduce to inputs/outputs only:
```
openapi-snapshot --url http://localhost:3000/api-docs/openapi.json --out openapi/backend_openapi.json --reduce paths,components
```

Outline profile (minimal path + schema refs):
```
openapi-snapshot --profile outline --out openapi/backend_openapi.outline.json
```
Note: `--reduce` is not supported with `--profile outline`.

Generate both full and outline snapshots in one run:
```
openapi-snapshot --out openapi/backend_openapi.json --outline-out openapi/backend_openapi.outline.json
```

Add auth header:
```
openapi-snapshot --url http://localhost:3000/api-docs/openapi.json --out openapi/backend_openapi.json --header "Authorization: Bearer TOKEN"
```

Print to stdout:
```
openapi-snapshot --url http://localhost:3000/api-docs/openapi.json --stdout
```

Minified output (single line):
```
openapi-snapshot --minify true --out openapi/backend_openapi.min.json
```

## Continuous update

Keep the file updated while you code:

```
openapi-snapshot watch
```

Defaults for `watch`:
- URL: `http://localhost:3000/api-docs/openapi.json`
- Output: `openapi/backend_openapi.json`
- Outline output: `openapi/backend_openapi.outline.json`
- Reduce: `paths,components`
- Interval: 2000ms
- Minify: `false`

If the default URL is unreachable and you're in a terminal, `watch` will prompt you for a port or full URL once and continue with that value.

Override anything if needed:
```
openapi-snapshot watch --url http://localhost:3000/api-docs/openapi.json --out openapi/backend_openapi.json --outline-out openapi/backend_openapi.outline.json --reduce paths,components --interval-ms 2000
```

Outline watch:
```
openapi-snapshot watch --profile outline --out openapi/backend_openapi.outline.json
```

Disable the default outline output:
```
openapi-snapshot watch --no-outline
```

Leave it running. It refreshes the snapshot files on the interval.

If you already run your backend separately, you can skip the restart and just run the exporter on a timer or on demand:
```
openapi-snapshot --url http://localhost:3000/api-docs/openapi.json --out openapi/backend_openapi.json --reduce paths,components
```

## Notes

- This tool fetches the spec from a running server; it does not generate OpenAPI from code.
- If your OpenAPI endpoint is protected, pass `--header` for auth.

## Release checklist

- Update `Cargo.toml` version.
- Run `cargo test`.
- Run `cargo publish`.
- Tag the release in git.
