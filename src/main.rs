use clap::Parser;
use openapi_snapshot::{
    build_output, run_watch, validate_config, write_output, AppError, Cli, Config, Mode,
};

fn main() {
    let cli = Cli::parse();
    let (config, mode) = match Config::from_cli(cli) {
        Ok(result) => result,
        Err(err) => exit_with_error(err),
    };

    if config.stdout && config.out.is_some() {
        eprintln!("--out is ignored because --stdout is set.");
    }

    if let Err(err) = validate_config(&config) {
        exit_with_error(err);
    }

    match mode {
        Mode::Snapshot => {
            let payload = match build_output(&config) {
                Ok(payload) => payload,
                Err(err) => exit_with_error(err),
            };

            if let Err(err) = write_output(&config, &payload) {
                exit_with_error(err);
            }
        }
        Mode::Watch { interval_ms } => {
            if let Err(err) = run_watch(&config, interval_ms) {
                exit_with_error(err);
            }
        }
    }
}

fn exit_with_error(err: AppError) -> ! {
    eprintln!("{err}");
    std::process::exit(err.exit_code());
}
