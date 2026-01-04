use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use ctrlc;

use crate::config::Config;
use crate::errors::AppError;
use crate::output::{build_outputs, write_outputs};

const MIN_INTERVAL_MS: u64 = 250;
const BACKOFF_MAX_MS: u64 = 10_000;

pub fn run_watch(config: &mut Config, interval_ms: u64) -> Result<(), AppError> {
    let shutdown = Arc::new(AtomicBool::new(false));
    install_ctrlc_handler(shutdown.clone());

    let base_interval = interval_ms.max(MIN_INTERVAL_MS);
    let mut prompted = false;
    let mut backoff_ms = base_interval;
    let mut consecutive_errors: u32 = 0;

    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        match build_outputs(config) {
            Ok(outputs) => {
                consecutive_errors = 0;
                backoff_ms = base_interval;
                if let Err(err) = write_outputs(config, &outputs) {
                    eprintln!("{err}");
                }
            }
            Err(err) => {
                if !prompted && config.url_from_default && err.is_url_related() {
                    if let Some(new_url) = prompt_for_url(&config.url)? {
                        eprintln!("Switching watch URL from default to '{new_url}' after prompt.");
                        config.url = new_url;
                        config.url_from_default = false;
                        prompted = true;
                        continue;
                    }
                    prompted = true;
                }
                consecutive_errors = consecutive_errors.saturating_add(1);
                backoff_ms = next_backoff(backoff_ms);
                eprintln!("{err}");
            }
        }

        let sleep_ms = if consecutive_errors == 0 {
            base_interval
        } else {
            backoff_ms
        }
        .max(MIN_INTERVAL_MS);

        if wait_with_shutdown(&shutdown, sleep_ms) {
            break;
        }
    }

    Ok(())
}

fn install_ctrlc_handler(flag: Arc<AtomicBool>) {
    let _ = ctrlc::set_handler(move || {
        flag.store(true, Ordering::SeqCst);
    });
}

fn wait_with_shutdown(shutdown: &Arc<AtomicBool>, sleep_ms: u64) -> bool {
    let sleep_duration = Duration::from_millis(sleep_ms);
    let slice = Duration::from_millis(50);
    let mut waited = Duration::from_millis(0);
    while waited < sleep_duration {
        if shutdown.load(Ordering::SeqCst) {
            return true;
        }
        let remaining = sleep_duration.saturating_sub(waited);
        let step = remaining.min(slice);
        thread::sleep(step);
        waited += step;
    }
    shutdown.load(Ordering::SeqCst)
}

fn next_backoff(current: u64) -> u64 {
    let doubled = current.saturating_mul(2);
    doubled.min(BACKOFF_MAX_MS)
}

pub fn maybe_prompt_for_url(config: &mut Config, err: &AppError) -> Result<bool, AppError> {
    if !config.url_from_default || !err.is_url_related() {
        return Ok(false);
    }
    if let Some(new_url) = prompt_for_url(&config.url)? {
        eprintln!("Switching URL from default to '{new_url}' after prompt.");
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
        return Some(format!("http://localhost:{trimmed}/api-docs/openapi.json"));
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

    #[test]
    fn backoff_clamps() {
        assert_eq!(next_backoff(250), 500);
        assert_eq!(next_backoff(5_000), 10_000);
        assert_eq!(next_backoff(20_000), 10_000);
    }
}

fn install_ctrlc_handler(shutdown: Arc<AtomicBool>) {
    if let Err(err) = ctrlc::set_handler(move || {
        shutdown.store(true, Ordering::SeqCst);
    }) {
        eprintln!("failed to install Ctrl+C handler: {err}");
    }
}

fn run_watch_loop(
    config: &mut Config,
    interval_ms: u64,
    shutdown: Arc<AtomicBool>,
) -> Result<(), AppError> {
    let mut prompted = false;
    let mut failures = 0usize;
    let base_interval = interval_ms.max(MIN_INTERVAL_MS);

    loop {
        if shutdown.load(Ordering::SeqCst) {
            return Ok(());
        }

        match build_outputs(config) {
            Ok(outputs) => {
                failures = 0;
                if let Err(err) = write_outputs(config, &outputs) {
                    failures += 1;
                    eprintln!("{err}");
                }
            }
            Err(err) => {
                if !prompted && config.url_from_default && err.is_url_related() {
                    if let Some(new_url) = prompt_for_url(&config.url)? {
                        config.url = new_url;
                        config.url_from_default = false;
                        prompted = true;
                        eprintln!("Switching to provided URL: {}", config.url);
                        continue;
                    }
                    prompted = true;
                }
                failures += 1;
                eprintln!("{err}");
            }
        }

        let sleep_ms = compute_backoff_ms(base_interval, failures);
        thread::sleep(Duration::from_millis(sleep_ms));
    }
}

pub fn maybe_prompt_for_url(config: &mut Config, err: &AppError) -> Result<bool, AppError> {
    if !config.url_from_default || !err.is_url_related() {
        return Ok(false);
    }
    if let Some(new_url) = prompt_for_url(&config.url)? {
        config.url = new_url;
        config.url_from_default = false;
        eprintln!("Switching to provided URL: {}", config.url);
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

fn compute_backoff_ms(base_interval: u64, failures: usize) -> u64 {
    let base = base_interval.max(MIN_INTERVAL_MS);
    if failures == 0 {
        return base;
    }
    let factor = 1u64.saturating_shl(failures.saturating_sub(1) as u32);
    let backoff = base.saturating_mul(factor);
    backoff.min(BACKOFF_MAX_MS)
}

fn normalize_user_url(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Some(format!("http://localhost:{trimmed}/api-docs/openapi.json"));
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

    #[test]
    fn backoff_grows_and_caps() {
        let base = 500;
        assert_eq!(compute_backoff_ms(base, 0), 500);
        assert_eq!(compute_backoff_ms(base, 1), 500);
        assert_eq!(compute_backoff_ms(base, 2), 1_000);
        assert_eq!(compute_backoff_ms(base, 3), 2_000);
        assert_eq!(compute_backoff_ms(base, 4), 4_000);
        assert_eq!(compute_backoff_ms(base, 5), 5_000);
        assert_eq!(compute_backoff_ms(base, 10), 5_000);
    }

    #[test]
    fn backoff_honors_min_interval() {
        let base = 100;
        assert_eq!(compute_backoff_ms(base, 0), MIN_INTERVAL_MS);
        assert_eq!(compute_backoff_ms(base, 2), MIN_INTERVAL_MS * 2);
    }
}
