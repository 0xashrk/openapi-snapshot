use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::time::Duration;

use crate::config::Config;
use crate::errors::AppError;

pub fn fetch_openapi(config: &Config) -> Result<Vec<u8>, AppError> {
    let client = Client::builder()
        .timeout(Duration::from_millis(config.timeout_ms))
        .build()
        .map_err(|err| AppError::Network(format!("client error: {err}")))?;

    let headers = build_headers(&config.headers)?;
    let response = client
        .get(&config.url)
        .headers(headers)
        .send()
        .map_err(|err| AppError::Network(format!("request failed: {err}")))?;

    let status = response.status();
    if !status.is_success() {
        return Err(AppError::Network(format!(
            "unexpected status: {status}"
        )));
    }

    response
        .bytes()
        .map(|bytes| bytes.to_vec())
        .map_err(|err| AppError::Network(format!("failed to read response: {err}")))
}

pub fn parse_json(bytes: &[u8]) -> Result<Value, AppError> {
    serde_json::from_slice(bytes).map_err(|err| AppError::Json(format!("invalid JSON: {err}")))
}

fn build_headers(raw_headers: &[String]) -> Result<HeaderMap, AppError> {
    let mut headers = HeaderMap::new();
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
