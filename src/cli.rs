use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

pub const DEFAULT_URL: &str = "http://localhost:3000/api-docs/openapi.json";
pub const DEFAULT_OUT: &str = "openapi/backend_openapi.json";
pub const DEFAULT_OUTLINE_OUT: &str = "openapi/backend_openapi.outline.json";
pub const DEFAULT_REDUCE: &str = "paths,components";
pub const DEFAULT_INTERVAL_MS: u64 = 2_000;

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputProfile {
    Full,
    Outline,
}

#[derive(Parser, Debug)]
#[command(
    name = "openapi-snapshot",
    version,
    about = "Fetch and save an OpenAPI JSON snapshot.",
    after_help = "Examples:\n  openapi-snapshot\n  openapi-snapshot watch\n  openapi-snapshot --out openapi/backend_openapi.json --outline-out openapi/backend_openapi.outline.json\n  openapi-snapshot --profile outline --out openapi/backend_openapi.outline.json\n  openapi-snapshot --url http://localhost:3000/api-docs/openapi.json --out openapi/backend_openapi.json\n  openapi-snapshot --minify true --out openapi/backend_openapi.min.json"
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
    pub outline_out: Option<PathBuf>,
    #[arg(long)]
    pub reduce: Option<String>,
    #[arg(long, value_enum, default_value_t = OutputProfile::Full)]
    pub profile: OutputProfile,
    #[arg(
        long,
        default_value_t = false,
        default_missing_value = "true",
        num_args(0..=1),
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
    #[arg(long, default_value_t = false)]
    pub no_outline: bool,
}
