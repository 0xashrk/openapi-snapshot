use std::io::{self, IsTerminal, Write};
use std::thread;
use std::time::Duration;

use crate::config::Config;
use crate::errors::AppError;
use crate::output::{build_outputs, write_outputs};

pub fn run_watch(config: &mut Config, interval_ms: u64) -> Result<(), AppError> {
    let mut prompted = false;
    loop {
        match build_outputs(config) {
            Ok(outputs) => {
                if let Err(err) = write_outputs(config, &outputs) {
                    eprintln!("{err}");
                }
            }
            Err(err) => {
                if !prompted && config.url_from_default && err.is_url_related() {
                    if let Some(new_url) = prompt_for_url(&config.url)? {
                        config.url = new_url;
                        config.url_from_default = false;
                        prompted = true;
                        continue;
                    }
                    prompted = true;
                }
                eprintln!("{err}");
            }
        }
        thread::sleep(Duration::from_millis(interval_ms.max(250)));
    }
}

pub fn maybe_prompt_for_url(config: &mut Config, err: &AppError) -> Result<bool, AppError> {
    if !config.url_from_default || !err.is_url_related() {
        return Ok(false);
    }
    if let Some(new_url) = prompt_for_url(&config.url)? {
        config.url = new_url;
        config.url_from_default = false;
        return Ok(true);
    }
    Ok(false)
}

fn prompt_for_url(default_url: &str) -> Result<Option<String>, AppError> {
    if !io::stdin().is_terminal() {
        return Ok(None);
    }

    let mut input = String::new();
    loop {
        eprint!("OpenAPI URL (default: {default_url}) - enter port or URL: ");
        io::stdout()
            .flush()
            .map_err(|err| AppError::Io(format!("failed to flush prompt: {err}")))?;
        input.clear();
        io::stdin()
            .read_line(&mut input)
            .map_err(|err| AppError::Io(format!("failed to read input: {err}")))?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        if let Some(url) = normalize_user_url(trimmed) {
            return Ok(Some(url));
        }
        eprintln!("Invalid input. Enter a port (e.g., 3000) or full URL.");
    }
}

fn normalize_user_url(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Some(format!(
            "http://localhost:{trimmed}/api-docs/openapi.json"
        ));
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }
    if trimmed.contains(':') {
        return Some(format!("http://{trimmed}/api-docs/openapi.json"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_user_url_accepts_port() {
        let url = normalize_user_url("3001").unwrap();
        assert_eq!(url, "http://localhost:3001/api-docs/openapi.json");
    }

    #[test]
    fn normalize_user_url_accepts_full_url() {
        let url = normalize_user_url("https://example.com/openapi.json").unwrap();
        assert_eq!(url, "https://example.com/openapi.json");
    }

    #[test]
    fn normalize_user_url_accepts_host_port() {
        let url = normalize_user_url("localhost:4000").unwrap();
        assert_eq!(url, "http://localhost:4000/api-docs/openapi.json");
    }

    #[test]
    fn normalize_user_url_rejects_invalid() {
        assert!(normalize_user_url("not a url").is_none());
    }
}
