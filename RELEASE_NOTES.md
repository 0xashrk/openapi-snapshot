# Release Notes

## v0.1.0

- One-shot snapshot: fetch OpenAPI JSON and write a minified file.
- Watch mode with defaults (localhost:3000, `openapi/backend_openapi.min.json`, reduce `paths,components`).
- Optional flags: `--reduce`, `--stdout`, `--header`, `--timeout-ms`.
- Auto-create output directories.
- Prompt for port or full URL if the default endpoint is unreachable (TTY only).
- Comprehensive CLI and integration tests.
