use std::path::PathBuf;

use crate::cli::{Cli, Command, OutputProfile, DEFAULT_OUT, DEFAULT_REDUCE, DEFAULT_URL};
use crate::errors::AppError;

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

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Snapshot,
    Watch { interval_ms: u64 },
}

#[derive(Debug)]
pub struct Config {
    pub url: String,
    pub url_from_default: bool,
    pub out: Option<PathBuf>,
    pub reduce: Vec<ReduceKey>,
    pub profile: OutputProfile,
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

        let reduce_value = match (&cli.common.reduce, mode, cli.common.profile) {
            (Some(value), _, _) => Some(value.as_str()),
            (None, Mode::Watch { .. }, OutputProfile::Full) => Some(DEFAULT_REDUCE),
            _ => None,
        };
        let reduce = match reduce_value {
            Some(value) => parse_reduce_list(value)?,
            None => Vec::new(),
        };

        let url_from_default = cli.common.url.is_none();
        let url = cli.common.url.unwrap_or_else(|| DEFAULT_URL.to_string());
        let out = if cli.common.stdout {
            cli.common.out
        } else {
            Some(cli.common.out.unwrap_or_else(|| PathBuf::from(DEFAULT_OUT)))
        };

        Ok((
            Self {
                url,
                url_from_default,
                out,
                reduce,
                profile: cli.common.profile,
                minify: cli.common.minify,
                timeout_ms: cli.common.timeout_ms,
                headers: cli.common.header,
                stdout: cli.common.stdout,
            },
            mode,
        ))
    }
}

pub fn validate_config(config: &Config) -> Result<(), AppError> {
    if !config.stdout && config.out.is_none() {
        return Err(AppError::Usage(
            "--out is required unless --stdout is set.".to_string(),
        ));
    }
    if config.profile == OutputProfile::Outline && !config.reduce.is_empty() {
        return Err(AppError::Usage(
            "--reduce is not supported with --profile outline.".to_string(),
        ));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{CommonArgs, WatchArgs};

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
    fn defaults_apply_for_watch_mode() {
        let cli = Cli {
            command: Some(Command::Watch(WatchArgs { interval_ms: 500 })),
            common: CommonArgs {
                url: None,
                out: None,
                reduce: None,
                profile: OutputProfile::Full,
                minify: true,
                timeout_ms: 10_000,
                header: Vec::new(),
                stdout: false,
            },
        };
        let (config, mode) = Config::from_cli(cli).unwrap();
        assert_eq!(config.url, DEFAULT_URL);
        assert!(config.url_from_default);
        assert_eq!(config.out.unwrap(), PathBuf::from(DEFAULT_OUT));
        assert_eq!(config.reduce, vec![ReduceKey::Paths, ReduceKey::Components]);
        assert!(matches!(mode, Mode::Watch { .. }));
    }
}
