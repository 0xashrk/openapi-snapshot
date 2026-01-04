use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::cli::OutputProfile;
use crate::config::{Config, ReduceKey};
use crate::errors::AppError;
use crate::fetch::{fetch_openapi, parse_json};
use crate::outline::outline_openapi;

#[derive(Debug)]
pub struct OutputPayloads {
    pub primary: String,
    pub outline: Option<String>,
}

pub fn build_output(config: &Config) -> Result<String, AppError> {
    Ok(build_outputs(config)?.primary)
}

pub fn build_outputs(config: &Config) -> Result<OutputPayloads, AppError> {
    let body = fetch_openapi(config)?;
    let json = parse_json(&body)?;
    match config.profile {
        OutputProfile::Full => {
            let mut full_value = json.clone();
            if !config.reduce.is_empty() {
                full_value = reduce_openapi(full_value, &config.reduce)?;
            }
            let primary = serialize_json(&full_value, config.minify)?;
            let outline = if config.outline_out.is_some() {
                let outline_value = outline_openapi(&json)?;
                Some(serialize_json(&outline_value, config.minify)?)
            } else {
                None
            };
            Ok(OutputPayloads { primary, outline })
        }
        OutputProfile::Outline => {
            let outline_value = outline_openapi(&json)?;
            let primary = serialize_json(&outline_value, config.minify)?;
            Ok(OutputPayloads {
                primary,
                outline: None,
            })
        }
    }
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

pub fn write_outputs(config: &Config, outputs: &OutputPayloads) -> Result<(), AppError> {
    if config.stdout {
        println!("{}", outputs.primary);
        return Ok(());
    }

    let out_path = config
        .out
        .as_ref()
        .ok_or_else(|| AppError::Usage("--out is required unless --stdout is set.".to_string()))?;
    write_atomic(out_path, &outputs.primary)?;

    if let Some(outline_payload) = outputs.outline.as_ref() {
        if let Some(outline_path) = config.outline_out.as_ref() {
            write_atomic(outline_path, outline_payload)?;
        }
    }

    Ok(())
}

fn reduce_openapi(value: Value, keys: &[ReduceKey]) -> Result<Value, AppError> {
    let object = value
        .as_object()
        .ok_or_else(|| AppError::Reduce("OpenAPI document must be a JSON object".to_string()))?;
    let mut reduced = serde_json::Map::new();
    for key in keys {
        let name = key.as_str();
        let entry = object
            .get(name)
            .ok_or_else(|| AppError::Reduce(format!("missing top-level key: {name}")))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reduce_openapi_keeps_only_requested_keys() {
        let input = json!({
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
        let input = json!({"paths": {"x": 1}});
        let err = reduce_openapi(input, &[ReduceKey::Components]).unwrap_err();
        assert!(matches!(err, AppError::Reduce(_)));
    }

    #[test]
    fn reduce_openapi_requires_object() {
        let input = json!(["not an object"]);
        let err = reduce_openapi(input, &[ReduceKey::Components]).unwrap_err();
        assert!(matches!(err, AppError::Reduce(_)));
    }
}
