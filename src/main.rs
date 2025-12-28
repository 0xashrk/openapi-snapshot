use clap::Parser;
use openapi_snapshot::{
    build_outputs, maybe_prompt_for_url, run_watch, validate_config, write_outputs, AppError, Cli,
    Config, Mode,
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
            let mut config = config;
            let outputs = match build_outputs(&config) {
                Ok(outputs) => outputs,
                Err(err) => {
                    if let Ok(true) = maybe_prompt_for_url(&mut config, &err) {
                        match build_outputs(&config) {
                            Ok(outputs) => outputs,
                            Err(err) => exit_with_error(err),
                        }
                    } else {
                        exit_with_error(err);
                    }
                }
            };

            if let Err(err) = write_outputs(&config, &outputs) {
                exit_with_error(err);
            }
        }
        Mode::Watch { interval_ms } => {
            let mut config = config;
            if let Err(err) = run_watch(&mut config, interval_ms) {
                exit_with_error(err);
            }
        }
    }
}

fn exit_with_error(err: AppError) -> ! {
    eprintln!("{err}");
    std::process::exit(err.exit_code());
}
