pub mod cli;
pub mod config;
pub mod errors;
pub mod fetch;
pub mod outline;
pub mod output;
pub mod watch;

pub use cli::{Cli, Command, CommonArgs, OutputProfile, WatchArgs};
pub use config::{parse_reduce_list, validate_config, Config, Mode, ReduceKey};
pub use errors::AppError;
pub use output::{build_output, write_output};
pub use watch::{maybe_prompt_for_url, run_watch};
