use clap::{Args, Parser, Subcommand};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_URL: &str = "http://localhost:3000/api-docs/openapi.json";
const DEFAULT_OUT: &str = "openapi/backend_openapi.min.json";
const DEFAULT_REDUCE: &str = "paths,components";
const DEFAULT_INTERVAL_MS: u64 = 2_000;

#[derive(Parser, Debug)]
#[command(
    name = "openapi-snapshot",
    version,
    about = "Fetch and save a minified OpenAPI JSON snapshot.",
    after_help = "Examples:\n  openapi-snapshot\n  openapi-snapshot watch\n  openapi-snapshot --url http://localhost:3000/api-docs/openapi.json --out openapi/backend_openapi.min.json"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
    #[command(flatten)]
    pub common: CommonArgs,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Watch(WatchArgs),
}

#[derive(Args, Debug, Clone)]
pub struct CommonArgs {
    #[arg(long)]
    pub url: Option<String>,
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long)]
    pub reduce: Option<String>,
    #[arg(
        long,
        default_value_t = true,
        value_parser = clap::builder::BoolishValueParser::new()
    )]
    pub minify: bool,
    #[arg(long, default_value_t = 10_000)]
    pub timeout_ms: u64,
    #[arg(long)]
    pub header: Vec<String>,
    #[arg(long)]
    pub stdout: bool,
}

#[derive(Args, Debug, Clone)]
pub struct WatchArgs {
    #[arg(long, default_value_t = DEFAULT_INTERVAL_MS)]
    pub interval_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Snapshot,
    Watch { interval_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReduceKey {
    Paths,
    Components,
}

impl ReduceKey {
    pub fn as_str(self) -> &'static str {
        match self {
            ReduceKey::Paths => "paths",
            ReduceKey::Components => "components",
        }
    }
}

#[derive(Debug)]
pub struct Config {
    pub url: String,
    pub out: Option<PathBuf>,
    pub reduce: Vec<ReduceKey>,
    pub minify: bool,
    pub timeout_ms: u64,
    pub headers: Vec<String>,
    pub stdout: bool,
}

impl Config {
    pub fn from_cli(cli: Cli) -> Result<(Self, Mode), AppError> {
        let mode = match cli.command {
            Some(Command::Watch(args)) => Mode::Watch {
                interval_ms: args.interval_ms,
            },
            None => Mode::Snapshot,
        };

        let reduce_value = match (&cli.common.reduce, mode) {
            (Some(value), _) => Some(value.as_str()),
            (None, Mode::Watch { .. }) => Some(DEFAULT_REDUCE),
            _ => None,
        };
        let reduce = match reduce_value {
            Some(value) => parse_reduce_list(value)?,
            None => Vec::new(),
        };

        let url = cli
            .common
            .url
            .unwrap_or_else(|| DEFAULT_URL.to_string());
        let out = if cli.common.stdout {
            cli.common.out
        } else {
            Some(cli.common.out.unwrap_or_else(|| PathBuf::from(DEFAULT_OUT)))
        };

        Ok((
            Self {
                url,
                out,
                reduce,
                minify: cli.common.minify,
                timeout_ms: cli.common.timeout_ms,
                headers: cli.common.header,
                stdout: cli.common.stdout,
            },
            mode,
        ))
    }
}

#[derive(Debug)]
pub enum AppError {
    Usage(String),
    Network(String),
    Json(String),
    Reduce(String),
    Io(String),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            AppError::Usage(_) => 1,
            AppError::Network(_) => 1,
            AppError::Json(_) => 2,
            AppError::Reduce(_) => 3,
            AppError::Io(_) => 4,
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Usage(msg)
            | AppError::Network(msg)
            | AppError::Json(msg)
            | AppError::Reduce(msg)
            | AppError::Io(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AppError {}

pub fn validate_config(config: &Config) -> Result<(), AppError> {
    if !config.stdout && config.out.is_none() {
        return Err(AppError::Usage(
            "--out is required unless --stdout is set.".to_string(),
        ));
    }
    Ok(())
}

pub fn build_output(config: &Config) -> Result<String, AppError> {
    let body = fetch_openapi(config)?;
    let mut json = parse_json(&body)?;
    if !config.reduce.is_empty() {
        json = reduce_openapi(json, &config.reduce)?;
    }
    serialize_json(&json, config.minify)
}

pub fn write_output(config: &Config, payload: &str) -> Result<(), AppError> {
    if config.stdout {
        println!("{payload}");
        return Ok(());
    }

    let out_path = config
        .out
        .as_ref()
        .ok_or_else(|| AppError::Usage("--out is required unless --stdout is set.".to_string()))?;
    write_atomic(out_path, payload)
}

pub fn parse_reduce_list(value: &str) -> Result<Vec<ReduceKey>, AppError> {
    if value.is_empty() {
        return Err(AppError::Reduce("reduce list cannot be empty".to_string()));
    }
    let mut out = Vec::new();
    for raw in value.split(',') {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.to_lowercase() != trimmed {
            return Err(AppError::Reduce(format!(
                "reduce values must be lowercase: {trimmed}"
            )));
        }
        match trimmed {
            "paths" => push_unique(&mut out, ReduceKey::Paths),
            "components" => push_unique(&mut out, ReduceKey::Components),
            _ => {
                return Err(AppError::Reduce(format!(
                    "unsupported reduce value: {trimmed}"
                )))
            }
        }
    }
    if out.is_empty() {
        return Err(AppError::Reduce("reduce list cannot be empty".to_string()));
    }
    Ok(out)
}

fn push_unique(items: &mut Vec<ReduceKey>, key: ReduceKey) {
    if !items.contains(&key) {
        items.push(key);
    }
}

fn fetch_openapi(config: &Config) -> Result<Vec<u8>, AppError> {
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

fn parse_json(bytes: &[u8]) -> Result<Value, AppError> {
    serde_json::from_slice(bytes).map_err(|err| AppError::Json(format!("invalid JSON: {err}")))
}

fn reduce_openapi(value: Value, keys: &[ReduceKey]) -> Result<Value, AppError> {
    let object = value.as_object().ok_or_else(|| {
        AppError::Reduce("OpenAPI document must be a JSON object".to_string())
    })?;
    let mut reduced = serde_json::Map::new();
    for key in keys {
        let name = key.as_str();
        let entry = object.get(name).ok_or_else(|| {
            AppError::Reduce(format!("missing top-level key: {name}"))
        })?;
        reduced.insert(name.to_string(), entry.clone());
    }
    Ok(Value::Object(reduced))
}

fn serialize_json(value: &Value, minify: bool) -> Result<String, AppError> {
    if minify {
        serde_json::to_string(value).map_err(|err| AppError::Json(format!("json error: {err}")))
    } else {
        serde_json::to_string_pretty(value)
            .map_err(|err| AppError::Json(format!("json error: {err}")))
    }
}

fn write_atomic(path: &Path, contents: &str) -> Result<(), AppError> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Io("output path has no parent directory".to_string()))?;
    if let Err(err) = fs::create_dir_all(parent) {
        return Err(AppError::Io(format!(
            "failed to create output directory: {err}"
        )));
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let temp_name = format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("openapi_snapshot"),
        timestamp
    );
    let temp_path = parent.join(temp_name);

    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)
        .map_err(|err| AppError::Io(format!("failed to create temp file: {err}")))?;

    if let Err(err) = file.write_all(contents.as_bytes()) {
        let _ = fs::remove_file(&temp_path);
        return Err(AppError::Io(format!("failed to write temp file: {err}")));
    }

    if let Err(err) = file.sync_all() {
        let _ = fs::remove_file(&temp_path);
        return Err(AppError::Io(format!("failed to flush temp file: {err}")));
    }

    if let Err(err) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(AppError::Io(format!("failed to move temp file: {err}")));
    }

    Ok(())
}

pub fn run_watch(config: &Config, interval_ms: u64) -> Result<(), AppError> {
    loop {
        match build_output(config) {
            Ok(payload) => {
                if let Err(err) = write_output(config, &payload) {
                    eprintln!("{err}");
                }
            }
            Err(err) => {
                eprintln!("{err}");
            }
        }
        thread::sleep(Duration::from_millis(interval_ms.max(250)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reduce_list_accepts_paths_components() {
        let keys = parse_reduce_list("paths,components").unwrap();
        assert_eq!(keys, vec![ReduceKey::Paths, ReduceKey::Components]);
    }

    #[test]
    fn parse_reduce_list_rejects_mixed_case() {
        let err = parse_reduce_list("Paths").unwrap_err();
        assert!(matches!(err, AppError::Reduce(_)));
    }

    #[test]
    fn reduce_openapi_keeps_only_requested_keys() {
        let input = serde_json::json!({
            "paths": {"x": 1},
            "components": {"y": 2},
            "extra": {"z": 3}
        });
        let output = reduce_openapi(input, &[ReduceKey::Components]).unwrap();
        assert!(output.get("paths").is_none());
        assert!(output.get("components").is_some());
        assert!(output.get("extra").is_none());
    }

    #[test]
    fn reduce_openapi_missing_key_is_error() {
        let input = serde_json::json!({"paths": {"x": 1}});
        let err = reduce_openapi(input, &[ReduceKey::Components]).unwrap_err();
        assert!(matches!(err, AppError::Reduce(_)));
    }

    #[test]
    fn defaults_apply_for_watch_mode() {
        let cli = Cli {
            command: Some(Command::Watch(WatchArgs { interval_ms: 500 })),
            common: CommonArgs {
                url: None,
                out: None,
                reduce: None,
                minify: true,
                timeout_ms: 10_000,
                header: Vec::new(),
                stdout: false,
            },
        };
        let (config, mode) = Config::from_cli(cli).unwrap();
        assert_eq!(config.url, DEFAULT_URL);
        assert_eq!(config.out.unwrap(), PathBuf::from(DEFAULT_OUT));
        assert_eq!(config.reduce, vec![ReduceKey::Paths, ReduceKey::Components]);
        assert!(matches!(mode, Mode::Watch { .. }));
    }
}
