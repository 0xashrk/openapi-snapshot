use std::thread;
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::{self, HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;

use crate::config::Config;
use crate::errors::AppError;

const USER_AGENT: &str = concat!("openapi-snapshot/", env!("CARGO_PKG_VERSION"));
const MAX_RETRIES: usize = 3;
const BASE_BACKOFF_MS: u64 = 100;
const MAX_BACKOFF_MS: u64 = 2_000;
const ERROR_SNIPPET_LIMIT: usize = 256;

pub fn fetch_openapi(config: &Config) -> Result<Vec<u8>, AppError> {
    let headers = build_headers(&config.headers)?;
    let client = Client::builder()
        .timeout(Duration::from_millis(config.timeout_ms))
        .default_headers(headers)
        .build()
        .map_err(|err| AppError::Network(format!("client error: {err}")))?;

    let mut backoff = BASE_BACKOFF_MS;
    let mut attempt = 0;
    loop {
        attempt += 1;
        match client.get(&config.url).send() {
            Ok(mut response) => {
                let status = response.status();
                if !status.is_success() {
                    let snippet = body_snippet(response.text().unwrap_or_default());
                    let message = format!("HTTP {status}: {snippet}");
                    if should_retry_status(status) && attempt < MAX_RETRIES {
                        sleep(backoff);
                        backoff = next_backoff(backoff);
                        continue;
                    }
                    return Err(AppError::Network(message));
                }

                match response.bytes() {
                    Ok(bytes) => return Ok(bytes.to_vec()),
                    Err(err) => {
                        if is_retryable_error(&err) && attempt < MAX_RETRIES {
                            sleep(backoff);
                            backoff = next_backoff(backoff);
                            continue;
                        }
                        return Err(AppError::Network(format!("failed to read response: {err}")));
                    }
                }
            }
            Err(err) => {
                if is_retryable_error(&err) && attempt < MAX_RETRIES {
                    sleep(backoff);
                    backoff = next_backoff(backoff);
                    continue;
                }
                return Err(AppError::Network(format!("request failed: {err}")));
            }
        }
    }
}

pub fn parse_json(bytes: &[u8]) -> Result<Value, AppError> {
    serde_json::from_slice(bytes).map_err(|err| AppError::Json(format!("invalid JSON: {err}")))
}

fn build_headers(raw_headers: &[String]) -> Result<HeaderMap, AppError> {
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(header::USER_AGENT, HeaderValue::from_static(USER_AGENT));

    for raw in raw_headers {
        let (name, value) = parse_header(raw)?;
        headers.insert(name, value);
    }
    Ok(headers)
}

fn parse_header(raw: &str) -> Result<(HeaderName, HeaderValue), AppError> {
    let mut split = raw.splitn(2, ':');
    let name = split
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Usage(format!("invalid header format: {raw}")))?;
    let value = split
        .next()
        .map(str::trim)
        .ok_or_else(|| AppError::Usage(format!("invalid header format: {raw}")))?;
    let header_name = HeaderName::from_bytes(name.as_bytes())
        .map_err(|_| AppError::Usage(format!("invalid header name: {name}")))?;
    let header_value = HeaderValue::from_str(value)
        .map_err(|_| AppError::Usage(format!("invalid header value for: {name}")))?;
    Ok((header_name, header_value))
}

fn is_retryable_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_body()
}

fn should_retry_status(status: reqwest::StatusCode) -> bool {
    status.as_u16() == 429 || status.is_server_error()
}

fn next_backoff(current: u64) -> u64 {
    (current.saturating_mul(2)).min(MAX_BACKOFF_MS)
}

fn sleep(duration_ms: u64) {
    thread::sleep(Duration::from_millis(duration_ms));
}

fn body_snippet(body: String) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return String::from("<empty body>");
    }
    let snippet: String = trimmed.chars().take(ERROR_SNIPPET_LIMIT).collect();
    if snippet.len() < trimmed.len() {
        format!("{snippet}…")
    } else {
        snippet
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::OutputProfile;
    use crate::config::Config;
    use httpmock::prelude::*;

    fn base_config(url: String) -> Config {
        Config {
            url,
            url_from_default: false,
            out: None,
            outline_out: None,
            reduce: Vec::new(),
            profile: OutputProfile::Full,
            minify: false,
            timeout_ms: 5_000,
            headers: Vec::new(),
            stdout: true,
        }
    }

    #[test]
    fn fetch_includes_default_and_custom_headers() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/openapi.json")
                .header("accept", "application/json")
                .header("user-agent", USER_AGENT)
                .header("authorization", "Bearer token");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"openapi":"3.0.3","paths":{},"components":{}}"#);
        });

        let mut config = base_config(server.url("/openapi.json"));
        config
            .headers
            .push("Authorization: Bearer token".to_string());

        let bytes = fetch_openapi(&config).unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["openapi"], serde_json::json!("3.0.3"));
        mock.assert_hits(1);
    }

    #[test]
    fn retries_on_server_error_then_succeeds() {
        let server = MockServer::start();
        let fail = server.mock(|when, then| {
            when.method(GET).path("/openapi.json");
            then.status(500).body("temporary");
        });
        let success = server.mock(|when, then| {
            when.method(GET).path("/openapi.json");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"openapi":"3.0.3","paths":{},"components":{}}"#);
        });

        let config = base_config(server.url("/openapi.json"));
        let bytes = fetch_openapi(&config).unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["openapi"], serde_json::json!("3.0.3"));
        assert!(fail.hits() >= 1);
        assert!(success.hits() >= 1);
    }

    #[test]
    fn fetch_surfaces_status_and_body_snippet() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/openapi.json");
            then.status(502).body("gateway down");
        });

        let config = base_config(server.url("/openapi.json"));
        let err = fetch_openapi(&config).unwrap_err();
        match err {
            AppError::Network(msg) => {
                assert!(msg.contains("502"));
                assert!(msg.contains("gateway down"));
            }
            other => panic!("expected network error, got {other:?}"),
        }
    }

    #[test]
    fn returns_error_with_status_and_snippet_when_retries_exhausted() {
        let server = MockServer::start();
        let fail = server.mock(|when, then| {
            when.method(GET).path("/openapi.json");
            then.status(500).body("server exploded");
        });

        let config = base_config(server.url("/openapi.json"));
        let err = fetch_openapi(&config).unwrap_err();
        let message = format!("{err}");
        assert!(message.contains("HTTP 500"));
        assert!(message.contains("server exploded"));
        fail.assert_hits(MAX_RETRIES);
    }

    #[test]
    fn stops_after_max_retries_and_returns_error() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/openapi.json");
            then.status(503).body("down");
        });

        let config = base_config(server.url("/openapi.json"));
        let err = fetch_openapi(&config).unwrap_err();
        assert!(format!("{err}").contains("HTTP 503"));
        mock.assert_hits(MAX_RETRIES);
    }

    #[test]
    fn error_includes_body_snippet() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/openapi.json");
            then.status(500).body("something went wrong in backend");
        });

        let config = base_config(server.url("/openapi.json"));
        let err = fetch_openapi(&config).unwrap_err();
        match err {
            AppError::Network(msg) => {
                assert!(msg.contains("500"));
                assert!(msg.contains("something went wrong in backend"));
            }
            other => panic!("expected network error, got {other:?}"),
        }
        mock.assert_hits(1);
    }
}
